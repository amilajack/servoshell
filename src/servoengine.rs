extern crate servo;

use servoview::{DrawableGeometry, EventLoopRiser};
use self::servo::config::servo_version;
use self::servo::servo_config::opts;
use self::servo::servo_config::prefs::{PrefValue, PREFS};
use self::servo::compositing::windowing::{WindowMethods, WindowEvent};
use self::servo::compositing::compositor_thread::{self, CompositorProxy, CompositorReceiver};
use self::servo::msg::constellation_msg::{self, Key};
use self::servo::servo_geometry::DeviceIndependentPixel;
use self::servo::euclid::{Point2D, Size2D};
use self::servo::euclid::scale_factor::ScaleFactor;
use self::servo::euclid::size::TypedSize2D;
use self::servo::script_traits::DevicePixel;
use self::servo::servo_url::ServoUrl;
use self::servo::net_traits::net_error_list::NetError;

use std::fmt;
use std::sync::mpsc;
use std::rc::Rc;
use std::cell::{Cell, RefCell};

pub use self::servo::style_traits::cursor::Cursor as ServoCursor;
pub type CompositorChannel = (Box<CompositorProxy + Send>, Box<CompositorReceiver>);


#[derive(Clone)]
pub enum ServoEvent {
    SetWindowInnerSize(u32, u32),
    SetWindowPosition(i32, i32),
    SetFullScreenState(bool),
    Present,
    TitleChanged(Option<String>),
    UnhandledURL(ServoUrl),
    URLChanged(ServoUrl),
    StatusChanged(Option<String>),
    LoadStart(bool, bool),
    LoadEnd(bool, bool, bool),
    LoadError(String),
    HeadParsed(ServoUrl),
    CursorChanged(ServoCursor),
    FaviconChanged(ServoUrl),
    Key(Option<char>, Key, constellation_msg::KeyModifiers),
}

pub enum FollowLinkPolicy {
    FollowAnyLink,
    FollowOriginalDomain,
}

impl fmt::Debug for ServoEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ServoEvent::SetWindowInnerSize(..) => write!(f, "SetWindowInnerSize"),
            ServoEvent::SetWindowPosition(..) => write!(f, "SetWindowPosition"),
            ServoEvent::SetFullScreenState(..) => write!(f, "SetFullScreenState"),
            ServoEvent::Present => write!(f, "Present"),
            ServoEvent::TitleChanged(..) => write!(f, "TitleChanged"),
            ServoEvent::UnhandledURL(..) => write!(f, "UnhandledURL"),
            ServoEvent::URLChanged(..) => write!(f, "URLChanged"),
            ServoEvent::StatusChanged(..) => write!(f, "StatusChanged"),
            ServoEvent::LoadStart(..) => write!(f, "LoadStart"),
            ServoEvent::LoadEnd(..) => write!(f, "LoadEnd"),
            ServoEvent::LoadError(..) => write!(f, "LoadError"),
            ServoEvent::HeadParsed(..) => write!(f, "HeadParsed"),
            ServoEvent::CursorChanged(..) => write!(f, "CursorChanged"),
            ServoEvent::FaviconChanged(..) => write!(f, "FaviconChanged"),
            ServoEvent::Key(..) => write!(f, "Key"),
        }
    }
}

pub fn configure_servo(url: &str) {
    let url = ServoUrl::parse(url).ok().unwrap(); // FIXME. What if fail?
    let mut opts = opts::default_opts();
    opts.headless = false;
    opts.url = Some(url);
    opts::set_defaults(opts);
    // FIXME: Pipeline creation fails is layout_threads pref not set
    PREFS.set("layout.threads", PrefValue::Number(1.0));
}

pub struct ServoEngine {
    // FIXME: it's annoying that event for servo are named "WindowEvent"
    events_for_servo: RefCell<Vec<WindowEvent>>,
    servo_browser: RefCell<servo::Browser<ServoCallbacks>>,
    callbacks: Rc<ServoCallbacks>,
}

impl ServoEngine {
    pub fn new(geometry: DrawableGeometry,
               riser: EventLoopRiser,
               url: &str,
               follow_link_policy: FollowLinkPolicy)
               -> ServoEngine {

        let url = ServoUrl::parse(url).ok().unwrap(); // FIXME. What if fail?

        let allowed_domain = match follow_link_policy {
            FollowLinkPolicy::FollowOriginalDomain => {
                Some(url.domain().unwrap().clone().to_owned())
            }
            FollowLinkPolicy::FollowAnyLink => None,
        };

        let callbacks = Rc::new(ServoCallbacks {
            event_queue: RefCell::new(Vec::new()),
            geometry: Cell::new(geometry),
            riser: riser,
            allowed_domain: allowed_domain,
        });

        println!("{}", servo_version());

        let mut servo = servo::Browser::new(callbacks.clone());

        servo.handle_events(vec![WindowEvent::InitializeCompositing]);

        ServoEngine {
            events_for_servo: RefCell::new(Vec::new()),
            servo_browser: RefCell::new(servo),
            callbacks: callbacks,
        }
    }

    pub fn get_events(&self) -> Vec<ServoEvent> {
        self.callbacks.get_events()
    }

    // pub fn reload(&self) {
    //     let event = WindowEvent::Reload;
    //     self.events_for_servo.borrow_mut().push(event);
    // }

    // pub fn go_back(&self) {
    //     let event = WindowEvent::Navigation(WindowNavigateMsg::Back);
    //     self.events_for_servo.borrow_mut().push(event);
    // }

    // pub fn go_fwd(&self) {
    //     let event = WindowEvent::Navigation(WindowNavigateMsg::Forward);
    //     self.events_for_servo.borrow_mut().push(event);
    // }

    // pub fn perform_mouse_move(&self, x: i32, y: i32) {
    //     let event = WindowEvent::MouseWindowMoveEventClass(TypedPoint2D::new(x as f32, y as f32));
    //     self.events_for_servo.borrow_mut().push(event);
    // }

    // pub fn perform_scroll(&self, x: i32, y: i32, dx: f32, dy: f32, phase: WindowTouchPhase) {
    //     let scroll_location = webrender_traits::ScrollLocation::Delta(TypedPoint2D::new(dx, dy));
    //     let phase = match phase {
    //         WindowTouchPhase::Started => TouchEventType::Down,
    //         WindowTouchPhase::Moved => TouchEventType::Move,
    //         WindowTouchPhase::Ended => TouchEventType::Up,
    //         WindowTouchPhase::Cancelled => TouchEventType::Cancel,
    //     };
    //     let event = WindowEvent::Scroll(scroll_location, TypedPoint2D::new(x, y), phase);
    //     self.events_for_servo.borrow_mut().push(event);
    // }

    // pub fn perform_click(&self,
    //              x: i32,
    //              y: i32,
    //              org_x: i32,
    //              org_y: i32,
    //              element_state: WindowElementState,
    //              mouse_button: WindowMouseButton,
    //              mouse_down_button: Option<WindowMouseButton>) {
    //     use self::servo::script_traits::MouseButton;
    //     let max_pixel_dist = 10f64;
    //     let event = match element_state {
    //         WindowElementState::Pressed => {
    //             MouseWindowEvent::MouseDown(MouseButton::Left,
    //                                         TypedPoint2D::new(x as f32, y as f32))
    //         }
    //         WindowElementState::Released => {
    //             let mouse_up_event = MouseWindowEvent::MouseUp(MouseButton::Left,
    //                                                            TypedPoint2D::new(x as f32,
    //                                                                              y as f32));
    //             match mouse_down_button {
    //                 None => mouse_up_event,
    //                 Some(but) if mouse_button == but => {
    //                     // Same button
    //                     let pixel_dist = Point2D::new(org_x, org_y) - Point2D::new(x, y);
    //                     let pixel_dist =
    //                         ((pixel_dist.x * pixel_dist.x + pixel_dist.y * pixel_dist.y) as f64)
    //                             .sqrt();
    //                     if pixel_dist < max_pixel_dist {
    //                         self.events_for_servo
    //                             .borrow_mut()
    //                             .push(WindowEvent::MouseWindowEventClass(mouse_up_event));
    //                         MouseWindowEvent::Click(MouseButton::Left,
    //                                                 TypedPoint2D::new(x as f32, y as f32))
    //                     } else {
    //                         mouse_up_event
    //                     }
    //                 }
    //                 Some(_) => mouse_up_event,
    //             }
    //         }
    //     };
    //     self.events_for_servo.borrow_mut().push(WindowEvent::MouseWindowEventClass(event));
    // }

    pub fn sync(&self) {
        let clone = self.events_for_servo.borrow().clone();
        self.events_for_servo.borrow_mut().clear();
        self.servo_browser.borrow_mut().handle_events(clone);
    }
}

struct ServoCallbacks {
    event_queue: RefCell<Vec<ServoEvent>>,
    geometry: Cell<DrawableGeometry>,
    riser: EventLoopRiser,
    allowed_domain: Option<String>,
}

impl ServoCallbacks {
    fn get_events(&self) -> Vec<ServoEvent> {
        // FIXME: ports/glutin/window.rs uses mem::replace. Should we too?
        // See: https://doc.rust-lang.org/core/mem/fn.replace.html
        let clone = self.event_queue.borrow().clone();
        self.event_queue.borrow_mut().clear();
        clone
    }
}

impl WindowMethods for ServoCallbacks {
    fn prepare_for_composite(&self, _width: usize, _height: usize) -> bool {
        true
    }

    fn supports_clipboard(&self) -> bool {
        // FIXME
        false
    }

    fn allow_navigation(&self, url: ServoUrl) -> bool {
        let allow = self.allowed_domain
            .as_ref()
            .map(|domain| url.domain().unwrap() == domain)
            .unwrap_or(false);
        if !allow {
            self.event_queue.borrow_mut().push(ServoEvent::UnhandledURL(url));
        }
        allow
    }

    fn create_compositor_channel(&self) -> CompositorChannel {
        let (sender, receiver) = mpsc::channel();
        (box ShellCompositorProxy {
             sender: sender,
             riser: self.riser.clone(),
         } as Box<CompositorProxy + Send>,
         box receiver as Box<CompositorReceiver>)
    }

    fn hidpi_factor(&self) -> ScaleFactor<f32, DeviceIndependentPixel, DevicePixel> {
        let scale_factor = self.geometry.get().hidpi_factor;
        ScaleFactor::new(scale_factor)
    }

    fn framebuffer_size(&self) -> TypedSize2D<u32, DevicePixel> {
        let scale_factor = self.geometry.get().hidpi_factor as u32;
        let (width, height) = self.geometry.get().inner_size;
        TypedSize2D::new(scale_factor * width, scale_factor * height)
    }

    fn size(&self) -> TypedSize2D<f32, DeviceIndependentPixel> {
        let (width, height) = self.geometry.get().inner_size;
        TypedSize2D::new(width as f32, height as f32)
    }

    fn client_window(&self) -> (Size2D<u32>, Point2D<i32>) {
        let (width, height) = self.geometry.get().inner_size;
        let (x, y) = self.geometry.get().position;
        (Size2D::new(width, height), Point2D::new(x as i32, y as i32))
    }

    // Events

    fn set_inner_size(&self, size: Size2D<u32>) {
        self.event_queue
            .borrow_mut()
            .push(ServoEvent::SetWindowInnerSize(size.width as u32, size.height as u32));
    }

    fn set_position(&self, point: Point2D<i32>) {
        self.event_queue.borrow_mut().push(ServoEvent::SetWindowPosition(point.x, point.y));
    }

    fn set_fullscreen_state(&self, state: bool) {
        self.event_queue.borrow_mut().push(ServoEvent::SetFullScreenState(state))
    }

    fn present(&self) {
        // FIXME: what is that for? Aren't we supposed to flush buffer everytime
        // CompositorProxy::send is called?
        self.event_queue.borrow_mut().push(ServoEvent::Present);
    }

    fn set_page_title(&self, title: Option<String>) {
        self.event_queue.borrow_mut().push(ServoEvent::TitleChanged(title));
    }

    fn set_page_url(&self, url: ServoUrl) {
        self.event_queue.borrow_mut().push(ServoEvent::URLChanged(url));
    }

    fn status(&self, status: Option<String>) {
        self.event_queue.borrow_mut().push(ServoEvent::StatusChanged(status));
    }

    fn load_start(&self, can_go_back: bool, can_go_forward: bool) {
        self.event_queue.borrow_mut().push(ServoEvent::LoadStart(can_go_back, can_go_forward));
    }

    fn load_end(&self, can_go_back: bool, can_go_forward: bool, root: bool) {
        self.event_queue.borrow_mut().push(ServoEvent::LoadEnd(can_go_back, can_go_forward, root));
    }

    fn load_error(&self, _: NetError, url: String) {
        // FIXME: never called by servo
        self.event_queue.borrow_mut().push(ServoEvent::LoadError(url));
    }

    fn head_parsed(&self, url: ServoUrl) {
        self.event_queue.borrow_mut().push(ServoEvent::HeadParsed(url));
    }

    fn set_cursor(&self, cursor: ServoCursor) {
        println!("set_cursor: {:?}", cursor);
        self.event_queue.borrow_mut().push(ServoEvent::CursorChanged(cursor));
    }

    fn set_favicon(&self, url: ServoUrl) {
        self.event_queue.borrow_mut().push(ServoEvent::FaviconChanged(url));
    }

    fn handle_key(&self, ch: Option<char>, key: Key, mods: constellation_msg::KeyModifiers) {
        self.event_queue.borrow_mut().push(ServoEvent::Key(ch, key, mods));
    }
}

struct ShellCompositorProxy {
    sender: mpsc::Sender<compositor_thread::Msg>,
    riser: EventLoopRiser,
}

impl CompositorProxy for ShellCompositorProxy {
    fn send(&self, msg: compositor_thread::Msg) {
        if let Err(err) = self.sender.send(msg) {
            println!("Failed to send response ({}).", err);
        }
        self.riser.rise()
    }

    fn clone_compositor_proxy(&self) -> Box<CompositorProxy + Send> {
        box ShellCompositorProxy {
            sender: self.sender.clone(),
            riser: self.riser.clone(),
        } as Box<CompositorProxy + Send>
    }
}
