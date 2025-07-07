use cosmic::{
    app::{Core, Task},
    iced::{
        self, Subscription,
        alignment::Vertical,
        futures::channel::mpsc::Sender,
        platform_specific::shell::commands::layer_surface::{self, Anchor, Layer},
        runtime::platform_specific::wayland::layer_surface::{
            IcedMargin, SctkLayerSurfaceSettings,
        },
        stream, task, window,
    },
    prelude::Element,
    widget,
};
use monitor::mpris;
use std::{error::Error, time::Duration};
use tokio::task::JoinHandle;
use update::Update;

mod config;
mod monitor;

fn main() -> cosmic::iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // TODO: what `is_daemon` do???
    let settings = cosmic::app::Settings::default()
        .no_main_window(true)
        .client_decorations(false);

    tracing::info!("app running");
    cosmic::app::run::<AppModel>(settings, ())?;
    tracing::info!("app end");

    //stdout_loop().unwrap();
    Ok(())
}

// INFO:
// 1. volume (pipewire)
// 2. media (mpris)
// 3. screen brightness (udev?)
async fn event_loop(
    sender: Sender<Message>,
) -> Box<[Result<JoinHandle<()>, Box<dyn Error + Send + Sync>>]> {
    Box::new([
        // TODO: this will open a window on start, but we don't want that
        monitor::pipewire::start(sender.clone(), |event| Message::Error(event))
            .await
            .map_err(|e| e.into()),
        monitor::mpris::start(sender, |event| {
            use mpris::Event;
            match event {
                Event::NewMethodCall => Message::OpenOrRefreshWindow,
                Event::Update(update) => Message::UpdateMedia(UpdateMedia::Update(update)),
                Event::RemoveProperties(properties) => {
                    Message::UpdateMedia(UpdateMedia::Remove(properties))
                }
                Event::Error(error) => Message::Error(error),
            }
        })
        .await
        .map_err(|e| e.into()),
    ])
}

struct AppModel {
    core: Core,
    window: Option<Window>,
    timeout: Duration,
    showing_layer: ShowingLayer,
    media_status: mpris::Properties,
    error_message: Option<String>,
}

struct Window {
    id: window::Id,
    close_timer_abort_handle: task::Handle,
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
        assert!(core.main_window_id().is_none());
        (
            Self {
                core,
                window: None,
                timeout: Duration::from_secs(2),
                showing_layer: ShowingLayer::default(),
                media_status: mpris::Properties::default(),
                error_message: None,
            },
            Task::none(),
        )
    }
    fn view(&self) -> Element<Self::Message> {
        match self.window {
            Some(Window { id, .. }) => self.view_window(id),
            None => widget::text("Main Window").into(),
        }
    }
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        tracing::info!("update: {:#?}", message);
        match message {
            Message::UpdateMedia(update) => {
                self.showing_layer = ShowingLayer::Media;
                match update {
                    UpdateMedia::Replace(properties) => self.media_status = properties,
                    UpdateMedia::Remove(properties) => {
                        self.media_status.remove(properties.as_slice())
                    }
                    UpdateMedia::Update(properties) => self.media_status.update(properties),
                }
                Task::none()
            }
            Message::Error(e) => {
                self.error_message = Some(format!("error: {e}"));
                Task::done(cosmic::Action::App(Message::OpenOrRefreshWindow))
            }
            Message::Clicked => {
                // TODO: stop timer
                Task::done(cosmic::Action::App(Message::CloseWindow))
            }
            Message::OpenOrRefreshWindow => {
                let timeout = self.timeout.clone();
                let (close_timer, handle) = Task::future(async move {
                    tokio::time::sleep(timeout).await;
                    tracing::info!("timeout!!!");
                    cosmic::Action::App(Message::CloseWindow)
                })
                .abortable();

                match self.window.take() {
                    Some(Window {
                        id,
                        close_timer_abort_handle,
                    }) => {
                        close_timer_abort_handle.abort();
                        self.window = Some(Window {
                            id,
                            close_timer_abort_handle: handle,
                        });
                        close_timer
                    }
                    None => {
                        let (window_id, open_window) = window::open(window::Settings::default());
                        let open_window =
                            layer_surface::get_layer_surface(SctkLayerSurfaceSettings {
                                id: window_id,
                                layer: Layer::Overlay,
                                anchor: Anchor::BOTTOM,
                                size: Some((Some(600), Some(100))), // TODO: avoid this
                                margin: IcedMargin {
                                    bottom: 100,
                                    ..Default::default()
                                },
                                ..Default::default()
                            })
                            .chain(open_window.map(|_| cosmic::Action::None));
                        // .chain(window::get_size(window_id).then(move |size| {
                        //     tracing::info!("window size: {}, {}", size.width, size.height);
                        //     layer_surface::set_size(
                        //         window_id,
                        //         Some(size.width as u32),
                        //         Some(size.height as u32),
                        //     )
                        // }));

                        self.window = Some(Window {
                            id: window_id,
                            close_timer_abort_handle: handle,
                        });
                        Task::batch([open_window, close_timer])
                    }
                }
            }
            Message::CloseWindow => match self.window.take() {
                Some(Window {
                    id,
                    close_timer_abort_handle,
                }) => {
                    tracing::info!("closing window {id}");
                    close_timer_abort_handle.abort();
                    window::close(id).chain(layer_surface::destroy_layer_surface(id))
                }
                None => {
                    tracing::info!("close window message received but there is no window opened");
                    Task::none()
                }
            },
        }
    }
    fn subscription(&self) -> Subscription<Self::Message> {
        // FIXME: some how start all monitors
        Subscription::run(|| {
            // TODO: is 100 the size of channel?
            stream::channel(100, async |sender| {
                let handles = event_loop(sender).await; // TODO: handle errors
                for handle in handles {
                    if let Err(e) = handle {
                        tracing::error!(e)
                    }
                }
            })
        })
    }
    fn view_window(&self, _id: window::Id) -> Element<Self::Message> {
        match self.showing_layer {
            ShowingLayer::Media => self.media_status_view(),
            ShowingLayer::Volume => self.volume_status_view(),
            ShowingLayer::None => widget::row().into(),
        }
    }
}

impl AppModel {
    fn media_status_view(&self) -> Element<Message> {
        let metadata = self.media_status.metadata.as_ref();
        let art = metadata
            .map(|x| x.art_url.as_ref())
            .flatten()
            .map(|x| widget::image(x));
        let title = metadata
            .map(|x| x.title.as_ref())
            .flatten()
            .map(|x| widget::text(x).size(22));
        let artist = metadata
            .map(|x| x.artist.as_ref())
            .flatten()
            .map(|x| widget::text(x.join(", ")));
        let length = metadata
            .map(|x| x.length.as_ref())
            .flatten()
            .map(|x| widget::text(x.to_string()));
        let playback = self.media_status.playback_status.as_ref().map(|x| {
            widget::text(match x {
                mpris::PlaybackStatus::Playing => "",
                mpris::PlaybackStatus::Paused => "",
                mpris::PlaybackStatus::Stopped => "",
            })
            .size(42)
        });

        widget::container(
            widget::row()
                .align_y(Vertical::Center)
                .spacing(8)
                .push_maybe(art)
                .push(
                    widget::column()
                        .push_maybe(title)
                        .push_maybe(artist)
                        .push_maybe(length),
                )
                .push_maybe(playback),
        )
        // .width(500)
        // .height(100)
        .padding(20)
        .style(|_| {
            widget::container::background(iced::Color::BLACK)
                .border(iced::Border::default().rounded(20))
        })
        .into()
    }
    fn volume_status_view(&self) -> Element<Message> {
        widget::text("Volume").into()
    }
}

#[derive(Debug, Clone)]
enum Message {
    UpdateMedia(UpdateMedia),
    Error(String),
    Clicked,
    OpenOrRefreshWindow,
    CloseWindow,
}

#[derive(Default)]
enum ShowingLayer {
    #[default]
    None,
    Media,
    Volume,
}

#[derive(Debug, Clone)]
enum UpdateMedia {
    Replace(mpris::Properties),
    Update(mpris::Properties),
    Remove(Vec<String>),
}
