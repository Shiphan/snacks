use cosmic::iced::futures::{SinkExt, channel::{mpsc::Sender, oneshot}, executor};
use pipewire::context::Context;
use pipewire::main_loop::MainLoop;
use pipewire::types::ObjectType;
use tokio::task::JoinHandle;

pub async fn start<T: Send + 'static>(
    sender: Sender<T>,
    map: impl Fn(String) -> T + Clone + Send + Sync + 'static,
) -> Result<JoinHandle<()>, pipewire::Error> {
    let (tx, rx) = oneshot::channel();

    let handle = tokio::task::spawn_blocking(move || {
        let mainloop = match MainLoop::new(None) {
            Ok(x) => x,
            Err(e) => {
                tx.send(e).unwrap();
                return;
            }
        };
        let context = match Context::new(&mainloop) {
            Ok(x) => x,
            Err(e) => {
                tx.send(e).unwrap();
                return;
            }
        };
        let core = match context.connect(None) {
            Ok(x) => x,
            Err(e) => {
                tx.send(e).unwrap();
                return;
            }
        };
        let registry = match core.get_registry() {
            Ok(x) => x,
            Err(e) => {
                tx.send(e).unwrap();
                return;
            }
        };
        drop(tx);

        let weak_mainloop = mainloop.clone().downgrade();
        // TODO: find the event to listen
        let _listener = registry
            .add_listener_local()
            .global(move |global| {
                let mut sender = sender.clone();
                match &global.type_ {
                    ObjectType::Metadata => {
                        let message = format!(
                            "got a node and the props is {{\n{}}}",
                            match global.props {
                                Some(x) => format!(
                                    "DictRef {{\n{}}}",
                                    x.iter()
                                        .map(|(x, y)| format!("\t{}: {}\n", x, y))
                                        .collect::<String>()
                                ),
                                None => "None".to_owned(),
                            }
                        );

                        if executor::block_on(sender.send(map(message))).is_err() {
                            if let Some(mainloop) = weak_mainloop.upgrade() {
                                mainloop.quit();
                            }
                        }
                    }
                    _ => (),
                    //other => tx
                    //    .send(format!("got: `{}` but i dont care", other))
                    //    .unwrap(),
                }
            })
            .register();
        mainloop.run()
    });

    match rx.await {
        Ok(e) => {
            handle.await.unwrap();
            Err(e)
        }
        Err(_) => Ok(handle)
    }
}
