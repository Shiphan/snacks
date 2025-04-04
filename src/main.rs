use cosmic::app::{Core, Task};
use cosmic::iced::advanced::graphics::futures::stream;
use cosmic::iced::futures::channel::mpsc::Sender;
use cosmic::iced::window;
use cosmic::iced::Subscription;
use cosmic::prelude::*;
use cosmic::widget;
use futures::SinkExt;
use std::time::Duration;
//use sctk::reexports::client::globals;
//use sctk::shell::wlr_layer::LayerShell;

mod config;
mod monitor;

fn main() -> cosmic::iced::Result {
    //let (globals, qh) = globals::registry_queue_init().unwrap();
    //let layer_shell = LayerShell::bind(&globals, &qh).unwrap();

    // TODO: what `is_daemon` do???
    let settings = cosmic::app::Settings::default()
        .no_main_window(true)
        .client_decorations(false);

    println!("app running");
    cosmic::app::run::<AppModel>(settings, ())?;
    println!("app end");

    //stdout_loop().unwrap();
    Ok(())
}

// INFO:
// 1. volume (pipewire)
// 2. media (mpris)
// 3. screen brightness (udev?)
async fn event_loop(mut sender: Sender<Message>) {
    let mut pipewire_receiver = match monitor::pipewire::receiver().await {
        Ok((_, pipewire_receiver)) => pipewire_receiver,
        Err(e) => {
            sender
                .send(Message::Error(format!("Error: {:?}", e)))
                .await
                .unwrap();
            return;
        }
    };
    let mut mpris_receiver = match monitor::mpris::receiver().await {
        Ok((_, mpris_receiver)) => mpris_receiver,
        Err(e) => {
            sender
                .send(Message::Error(format!("Error: {:?}", e)))
                .await
                .unwrap();
            return;
        }
    };

    let mpris_handle = async {
        loop {
            sender
                .start_send(match mpris_receiver.recv().await {
                    Some(monitor::mpris::Update::Other(message)) => Message::Update(message),
                    Some(v) => Message::Update(format!("{v:?}")),
                    None => break,
                })
                .unwrap();
        }
    };
    tokio::join!(mpris_handle);

    // macro_rules! join_receiver {
    //     ( sender = $x:expr, $( $y:expr ), * ) => {
    //         tokio::join!($({
    //             async { loop {
    //                 match $y.recv().await {
    //                     Some(v) => $x
    //                         .clone()
    //                         .start_send(Message::Update(format!("{v:?}")))
    //                         .unwrap(),
    //                     None => break,
    //                 };
    //             } }
    //         },)*)
    //     };
    // }
    // let _ = join_receiver!(sender = sender, mpris_receiver);
    //let _ = join_receiver!(sender = sender, pipewire_receiver, mpris_receiver);
}

struct AppModel {
    core: Core,
    window: Option<window::Id>,
    c: u32,
    list: Vec<Result<String, String>>,
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "snacks";

    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }
    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let app = Self {
            window: core.main_window_id(),
            core,
            c: 0,
            list: vec![Ok("init".to_owned())],
        };
        println!("window: {:?}", app.window);
        (app, Task::none())
    }
    // TODO: hide the header bar
    fn view(&self) -> Element<Self::Message> {
        let mut list = widget::ListColumn::default();
        for item in self.list.iter().rev() {
            list = match item {
                Ok(s) => list.add(widget::text(s)),
                Err(e) => list.add(widget::warning(e)),
            };
        }
        widget::scrollable(list).into()
    }
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        println!("update: {:?}", message);
        match message {
            Message::Update(s) => {
                self.c += 1;
                self.list.push(Ok(s));
                Task::done(cosmic::app::Message::App(Message::Refresh))
            }
            Message::Error(e) => {
                self.list.push(Err(e));
                Task::done(cosmic::app::Message::App(Message::Refresh))
            }
            Message::Clicked => {
                // TODO: stop timer
                Task::done(cosmic::app::Message::App(Message::CloseWindow))
            }
            Message::Refresh => {
                // TODO: st
                Task::done(cosmic::app::Message::App(Message::OpenWindow))
            }
            Message::OpenWindow => {
                if self.window.is_none() {
                    let (window_id, task) = window::open(window::Settings::default());
                    self.window = Some(window_id);
                    task.map(|_| cosmic::app::Message::None)
                } else {
                    Task::none()
                }
            }
            Message::CloseWindow => {
                if let Some(self_window) = self.window {
                    self.window = None;
                    window::close(self_window)
                } else {
                    Task::none()
                }
            }
        }
    }
    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::run(|| stream::channel(100, event_loop)) // TODO: what is "size" (100)
    }
    fn view_window(&self, id: cosmic::iced::window::Id) -> Element<Self::Message> {
        let mut list = widget::ListColumn::default();
        for item in self.list.iter().rev() {
            list = match item {
                Ok(s) => list.add(widget::text(s)),
                Err(e) => list.add(widget::warning(e)),
            };
        }
        widget::scrollable(list).into()
    }
}

struct Timer {
    duration: Duration,
}

impl Timer {
    fn new(duration: Duration) -> Self {
        todo!()
    }
    fn start(&mut self) {}
}

#[derive(Debug, Clone)]
enum Message {
    Update(String),
    Error(String),
    Clicked,
    Refresh,
    OpenWindow,
    CloseWindow,
}
