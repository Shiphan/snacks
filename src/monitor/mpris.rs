use cosmic::{
    iced::futures::SinkExt,
    iced_futures::futures::{TryStreamExt, channel::mpsc::Sender},
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use update::macros::Update;
use zbus::{
    Connection, MatchRule, MessageStream, Proxy,
    zvariant::{self, DeserializeDict, SerializeDict, Type},
};

#[derive(Debug)]
pub enum Event {
    NewMethodCall,
    Update(Properties),
    RemoveProperties(Vec<String>),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum Update {
    PlaybackStatus(PlaybackStatus),
    LoopStatus(LoopStatus),
    Shuffle(bool),
    Metadata(Metadata),
}

#[derive(Serialize, Deserialize, Type, Clone, Debug, Default, PartialEq)]
#[zvariant(signature = "s")]
pub enum PlaybackStatus {
    Playing,
    Paused,
    #[default]
    Stopped,
}

#[derive(Serialize, Deserialize, Type, Clone, Debug, Default, PartialEq)]
#[zvariant(signature = "s")]
pub enum LoopStatus {
    #[default]
    None,
    Track,
    Playlist,
}

pub async fn start<T: Send + 'static>(
    mut sender: Sender<T>,
    map: impl Fn(Event) -> T + Clone + Send + Sync + 'static,
) -> Result<JoinHandle<()>, zbus::Error> {
    let send = async move |event| {
        if let Err(e) = sender.send(map(event)).await {
            tracing::error!("Cannot send to sender: {e}");
        }
    };
    let monitor_method_call = monitor_method_call(send.clone()).await?;
    let monitor_properties_change = monitor_properties_change(send).await?;
    Ok(tokio::spawn(async {
        tokio::join!(monitor_method_call, monitor_properties_change,);
    }))
}

async fn monitor_method_call(
    mut send: impl AsyncFnMut(Event) -> () + Clone,
) -> Result<impl Future<Output = ()>, zbus::Error> {
    let connection = Connection::session().await?;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::MethodCall)
        .interface("org.mpris.MediaPlayer2.Player")?
        // .path_namespace("/org/mpris/MediaPlayer2")?
        .build();
    tracing::info!("the match rule: {}", rule.to_string());

    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus.Monitoring",
    )
    .await?;
    let _: () = proxy.call("BecomeMonitor", &(vec![&rule], 0u32)).await?;

    let mut stream = MessageStream::for_match_rule(rule, &connection, None).await?;

    Ok(async move {
        loop {
            let event = match stream.try_next().await {
                Ok(Some(v)) => {
                    if v.header().member().is_some() {
                        Some(Event::NewMethodCall)
                    } else {
                        tracing::error!("a method call but no member (so no method): {v:#?}");
                        None
                    }
                }
                Ok(None) => {
                    tracing::info!("message stream ended");
                    break;
                }
                Err(e) => Some(Event::Error(format!("error from stream: {e}"))),
            };
            if let Some(event) = event {
                send(event).await;
            }
        }
    })
}

async fn monitor_properties_change(
    mut send: impl AsyncFnMut(Event) -> () + Clone + Send,
) -> Result<impl Future<Output = ()>, zbus::Error> {
    let connection = Connection::session().await?;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        // .path_namespace("/org/mpris/MediaPlayer2")?
        .build();
    tracing::info!("the match rule: {}", rule.to_string());

    // let proxy = Proxy::new(
    //     connection,
    //     "org.freedesktop.DBus",
    //     "/org/freedesktop/DBus",
    //     "org.freedesktop.DBus.Monitoring",
    // )
    // .await?;
    // let _: () = proxy.call("BecomeMonitor", &(vec![&rule], 0u32)).await?;

    let mut stream = MessageStream::for_match_rule(rule, &connection, None).await?;

    // TODO: monitor properties change
    Ok(async move {
        loop {
            match stream.try_next().await {
                Ok(Some(v)) => match v.body().deserialize::<PropertiesChanged>() {
                    Ok(body) => {
                        let event = Event::Update(body.changed_properties);
                        send(event).await;

                        if !body.invalidated_properties.is_empty() {
                            let event = Event::RemoveProperties(body.invalidated_properties);
                            send(event).await;
                        }
                    }
                    Err(e) => {
                        send(Event::Error(format!("deserialize error: {e} ({e:#?})"))).await;
                    }
                },
                Ok(None) => {
                    tracing::info!("message stream ended");
                    break;
                }
                Err(e) => {
                    tracing::error!("error: {e}");
                    break;
                }
            }
        }
    })
}

#[derive(Serialize, Deserialize, Type, Debug)]
struct PropertiesChanged {
    interface_name: String,
    changed_properties: Properties,
    invalidated_properties: Vec<String>,
}

#[derive(SerializeDict, DeserializeDict, Type, Clone, Debug, Default, Update)]
#[zvariant(signature = "a{sv}", rename_all = "PascalCase")]
pub struct Properties {
    pub playback_status: Option<PlaybackStatus>,
    pub loop_status: Option<LoopStatus>,
    pub rate: Option<f64>,
    pub shuffle: Option<bool>,
    pub metadata: Option<Metadata>,
    pub volume: Option<f64>,
    pub position: Option<i64>,
    pub minimum_rate: Option<f64>,
    pub maxinimum_rate: Option<f64>,
    pub can_go_next: Option<bool>,
    pub can_go_previous: Option<bool>,
    pub can_play: Option<bool>,
    pub can_pause: Option<bool>,
    pub can_seek: Option<bool>,
    pub can_control: Option<bool>,
}

#[derive(SerializeDict, DeserializeDict, Type, PartialEq, Debug, Clone, Default)]
#[zvariant(signature = "a{sv}")]
pub struct Metadata {
    #[zvariant(rename = "mpris:trackid")]
    pub trackid: Option<zvariant::OwnedObjectPath>,
    #[zvariant(rename = "mpris:length")]
    pub length: Option<i64>,
    #[zvariant(rename = "mpris:artUrl")]
    pub art_url: Option<String>,

    #[zvariant(rename = "xesam:album")]
    pub album: Option<String>,
    #[zvariant(rename = "xesam:albumArtist")]
    pub album_artist: Option<Vec<String>>,
    #[zvariant(rename = "xesam:artist")]
    pub artist: Option<Vec<String>>,
    // #[zvariant(rename = "xesam:asText")]
    // pub as_text: Option<String>,
    // #[zvariant(rename = "xesam:audioBPM")]
    // pub audio_bpm: Option<i32>,
    // #[zvariant(rename = "xesam:autoRating")]
    // pub auto_rating: Option<f64>,
    // #[zvariant(rename = "xesam:comment")]
    // pub comment: Option<Vec<String>>,
    // #[zvariant(rename = "xesam:composer")]
    // pub composer: Option<Vec<String>>,
    // #[zvariant(rename = "xesam:contentCreated")]
    // pub content_created: Option<String>,
    // #[zvariant(rename = "xesam:discNumber")]
    // pub disc_number: Option<i32>,
    // #[zvariant(rename = "xesam:firstUsed")]
    // pub first_used: Option<String>,
    // #[zvariant(rename = "xesam:genre")]
    // pub genre: Option<Vec<String>>,
    // #[zvariant(rename = "xesam:lastUsed")]
    // pub last_used: Option<String>,
    // #[zvariant(rename = "xesam:lyricist")]
    // pub lyricist: Option<Vec<String>>,
    #[zvariant(rename = "xesam:title")]
    pub title: Option<String>,
    // #[zvariant(rename = "xesam:trackNumber")]
    // pub track_number: Option<i32>,
    // #[zvariant(rename = "xesam:url")]
    // pub url: Option<String>,
    // #[zvariant(rename = "xesam:useCount")]
    // pub use_count: Option<i32>,
    // #[zvariant(rename = "xesam:userRating")]
    // pub user_rating: Option<f32>,
}
