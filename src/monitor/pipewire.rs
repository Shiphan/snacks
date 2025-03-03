use tokio::sync::mpsc::{
    unbounded_channel as tokio_unbounded_channel, UnboundedReceiver as TokioUnboundedReceiver,
    UnboundedSender as TokioUnboundedSender,
};
use tokio::task::JoinHandle;

pub async fn receiver() -> Result<(JoinHandle<()>, TokioUnboundedReceiver<String>), pipewire::Error>
{
    use pipewire::context::Context;
    use pipewire::main_loop::MainLoop;
    use pipewire::types::ObjectType;
    use std::sync::mpsc;

    let (tx, rx) = tokio_unbounded_channel();
    let (tx2, rx2) = mpsc::channel();

    let handle = tokio::task::spawn_blocking(move || {
        let mainloop = match MainLoop::new(None) {
            Ok(x) => x,
            Err(e) => {
                tx2.send(Some(e)).unwrap();
                return;
            }
        };
        let context = match Context::new(&mainloop) {
            Ok(x) => x,
            Err(e) => {
                tx2.send(Some(e)).unwrap();
                return;
            }
        };
        let core = match context.connect(None) {
            Ok(x) => x,
            Err(e) => {
                tx2.send(Some(e)).unwrap();
                return;
            }
        };
        let registry = match core.get_registry() {
            Ok(x) => x,
            Err(e) => {
                tx2.send(Some(e)).unwrap();
                return;
            }
        };
        tx2.send(None).unwrap();

        // TODO: find the event to listen
        let _listener = registry
            .add_listener_local()
            .global(move |global| match &global.type_ {
                ObjectType::Metadata => {
                    tx.send(format!(
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
                    ))
                    .unwrap();
                }
                _ => (),
                //other => tx
                //    .send(format!("got: `{}` but i dont care", other))
                //    .unwrap(),
            })
            .register();

        mainloop.run()
    });

    match rx2.recv().unwrap() {
        Some(e) => Err(e),
        None => Ok((handle, rx)),
    }
}
