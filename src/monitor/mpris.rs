use futures::TryStreamExt;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::task::JoinHandle;
use zbus::{Connection, MatchRule, MessageStream, Proxy};

#[derive(Debug)]
pub enum Update {
    Playing,
    Paused,
    Stopped,
    Error(String),
    Other(String),
}

pub async fn receiver() -> Result<(JoinHandle<()>, UnboundedReceiver<Update>), zbus::Error> {
    let (tx, rx) = mpsc::unbounded_channel();

    let connection = Connection::session().await?;

    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::MethodCall)
        .interface("org.mpris.MediaPlayer2.Player")?
        // .path_namespace("/org/mpris/MediaPlayer2")?
        .build();
    println!("the match rule: {}", rule.to_string());

    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus.Monitoring",
    )
    .await?;
    let _: () = proxy.call("BecomeMonitor", &(vec![&rule], 0u32)).await?;

    let mut stream = MessageStream::for_match_rule(rule, &connection, None).await?;

    let handle = tokio::spawn(async move {
        loop {
            let message = match stream.try_next().await {
                Ok(Some(v)) => {
                    match v.header().member().map(|x| x.as_str()) {
                        Some("Play") | Some("PlayPause") => Update::Playing,
                        _ => Update::Other(format!(
                            "ok: \n\theader: {:?}\n\tbody: {:?}",
                            v.header(),
                            v.body(),
                            // v.body().deserialize::<zbus::zvariant::Structure>()
                        )),
                    }
                }
                Ok(None) => {
                    println!("message stream ended");
                    break;
                }
                Err(e) => Update::Error(format!("err: {:#?}", e)),
            };
            if let Err(e) = tx.send(message) {
                eprintln!("error: {e}");
                break;
            }
        }
    });

    Ok((handle, rx))
}
