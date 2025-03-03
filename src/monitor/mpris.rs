use futures::TryStreamExt;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::task::JoinHandle;
use zbus::{Connection, MatchRule, MessageStream};

pub async fn receiver() -> Result<(JoinHandle<()>, UnboundedReceiver<String>), zbus::Error> {
    let (tx, rx) = mpsc::unbounded_channel();

    let connection = Connection::session().await?;

    let rule = MatchRule::builder()
        //.msg_type(zbus::message::Type::Signal)
        //.sender("org.freedesktop.DBus")?
        //.interface("org.mpris")?
        .path_namespace("/org/mpris/MediaPlayer2")?
        //.member("NameOwnerChanged")?
        //.add_arg("org.freedesktop.zbus.MatchRuleStreamTest42")?
        .build();
    let mut stream = MessageStream::for_match_rule(rule, &connection, Some(5)).await?;

    let handle = tokio::spawn(async move {
        loop {
            let message = match stream.try_next().await {
                Ok(Some(v)) => format!(
                    "ok: \n\theader: {:?}\n\tbody: {:?}",
                    v.header(),
                    v.body().deserialize::<zbus::zvariant::Structure>().unwrap()
                ),
                Ok(None) => break,
                Err(e) => format!("err: {:?}", e),
            };
            tx.send(message).unwrap();
        }
    });

    Ok((handle, rx))
}
