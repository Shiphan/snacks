use cosmic::app::{Core, Task};
use cosmic::iced::advanced::graphics::futures::stream;
use cosmic::iced::{self, Subscription, task, window};
use cosmic::iced_futures::futures::SinkExt;
use cosmic::iced_futures::futures::channel::mpsc::Sender;
use cosmic::prelude::Element;
use cosmic::widget;
use monitor::mpris::Properties;
use std::time::Duration;
use update::Update;

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
        Ok(mpris_receiver) => mpris_receiver,
        Err(e) => {
            sender
                .send(Message::Error(format!("Error: {:?}", e)))
                .await
                .unwrap();
            return;
        }
    };

    let mpris_handle = async {
        use monitor::mpris::Event;
        loop {
            sender
                .start_send(match mpris_receiver.recv().await {
                    Some(Event::NewMethodCall) => Message::OpenWindow,
                    Some(Event::Update(update)) => {
                        Message::UpdateMedia(UpdateMedia::Update(update))
                    }
                    Some(Event::RemoveProperties(properties)) => {
                        Message::UpdateMedia(UpdateMedia::Remove(properties))
                    }
                    Some(Event::Error(error)) => Message::Error(error),
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
    timeout: Duration,
    timer_abort_handle: Option<task::Handle>,
    media_status: monitor::mpris::Properties,
    error_message: Option<String>,
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
            timeout: Duration::from_secs(2),
            timer_abort_handle: None,
            media_status: Properties::default(),
            error_message: None,
        };
        println!("window: {:?}", app.window);
        (app, Task::none())
    }
    fn view(&self) -> Element<Self::Message> {
        widget::row().into()
    }
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        println!("update: {:?}", message);
        match message {
            Message::UpdateMedia(update) => {
                match update {
                    UpdateMedia::Replace(properties) => self.media_status = properties,
                    UpdateMedia::Remove(properties) => {
                        self.media_status.remove(properties.as_slice())
                    }
                    UpdateMedia::Update(properties) => self.media_status.update(properties),
                }
                println!("media status: {:#?}", self.media_status);
                Task::none()
            }
            Message::Error(e) => {
                self.error_message = Some(format!("error: {e}"));
                Task::done(cosmic::Action::App(Message::OpenWindow))
            }
            Message::Clicked => {
                // TODO: stop timer
                Task::done(cosmic::Action::App(Message::CloseWindow))
            }
            Message::OpenWindow => {
                let timeout = self.timeout.clone();
                let (close_timer, handle) = Task::future(async move {
                    tokio::time::sleep(timeout).await;
                    println!("timeout!!!");
                    cosmic::Action::App(Message::CloseWindow)
                })
                .abortable();
                if let Some(old_handle) = self.timer_abort_handle.replace(handle) {
                    old_handle.abort();
                };

                let open_window = match self.window {
                    Some(_) => Task::none(),
                    None => {
                        let (window_id, open_window) = window::open(window::Settings::default());
                    self.window = Some(window_id);
                        open_window.map(|_| cosmic::Action::None)
                }
                };
                Task::batch([open_window, close_timer])
            }
            Message::CloseWindow => {
                let Some(self_window) = self.window else {
                    return Task::none();
                };
                self.window = None;
                if let Some(old_handle) = self.timer_abort_handle.take() {
                    old_handle.abort();
                }
                window::close(self_window)
            }
        }
    }
    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::run(|| stream::channel(100, event_loop)) // TODO: what is "size" (100)
    }
    fn view_window(&self, id: cosmic::iced::window::Id) -> Element<Self::Message> {
        widget::container(
            widget::row()
                .padding(6)
                .push(widget::text(format!(
                    "{:?}",
                    self.media_status.playback_status
                )))
                .push(widget::horizontal_space())
                .push(
                    widget::column()
                        .push(widget::text(
                            self.media_status
                                .metadata
                                .as_ref()
                                .map(|x| x.title.as_ref())
                                .flatten()
                                .map(|x| x.as_str())
                                .unwrap_or("NO_TITLE"),
                        ))
                        .push(widget::text(
                            self.media_status
                                .metadata
                                .as_ref()
                                .map(|x| x.artist.as_ref())
                                .flatten()
                                .map(|x| x.join(", "))
                                .unwrap_or("NO_ARTIST".to_owned()),
                        )),
                )
                .push(widget::horizontal_space())
                .push(
                    widget::column()
                        .push(widget::text(format!(
                            "loop: {:?}",
                            self.media_status.loop_status
                        )))
                        .push(widget::text(format!(
                            "random: {:?}",
                            self.media_status.shuffle
                        ))),
                ),
        )
        .padding(12)
        .style(|_| widget::container::background(iced::Color::BLACK))
        .into()
    }
}

#[derive(Debug, Clone)]
enum Message {
    UpdateMedia(UpdateMedia),
    Error(String),
    Clicked,
    OpenWindow,
    CloseWindow,
}

#[derive(Debug, Clone)]
enum UpdateMedia {
    Replace(Properties),
    Update(Properties),
    Remove(Vec<String>),
}
