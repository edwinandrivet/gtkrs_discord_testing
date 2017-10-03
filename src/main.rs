extern crate gtk;
extern crate gdk;
extern crate glib;
extern crate pango;
extern crate discord;

use std::env;
use std::thread;
use std::io;
use std::sync::{Arc, Mutex};

use gtk::prelude::*;

use gdk::prelude::*;
use gdk::enums::key;

use discord::Discord;
use discord::model::Event;
use discord::model::ChannelId;
use std::sync::mpsc::channel;

#[derive(Debug, Clone)]
struct State {
    current_nickname: &'static str,
}

impl State {
    fn new() -> State {
        State {
            current_nickname: "",
        }
    }
    fn set_current_nickname(&mut self, new_nickname: &'static str) {
        self.current_nickname = new_nickname;
    }
}

#[derive(Debug, Clone)]
struct Win {
    window: gtk::Window,
    css_provider: gtk::CssProvider,
    display: gdk::Display,
    screen: gdk::Screen,
    geometry: gdk::Rectangle,
    default_width: i32,
    default_height: i32,
    main_box: gtk::Box,
}

impl Win {
    fn new() -> Win {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        let css_provider = gtk::CssProvider::new();
        let display = gdk::Display::get_default().unwrap();
        let screen = gdk::Display::get_default_screen(&display);
        let monitor = screen.get_monitor_at_window(&screen.get_active_window().unwrap());
        let geometry = screen.get_monitor_geometry(monitor);
        let (default_width, default_height) = (geometry.width / 2, geometry.height / 2);
        let main_box = gtk::Box::new(gtk::Orientation::Vertical, 25);

        window.set_title("Discord");
        window.set_position(gtk::WindowPosition::Center);
        window.set_default_size(default_width, default_height);

        window.connect_delete_event(|_, _| {
            gtk::main_quit();
            Inhibit(false)
        });

        css_provider.connect_parsing_error(|_, _, error| {
            println!("{:?}", error);
        });
        let ret = css_provider.load_from_path("test.css");
        match ret {
            Ok(ret) => println!("Worked? {:?}", ret),
            Err(error) => println!("Worked? {}", error),
        }

        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        /*        window.connect_configure_event(|_, event| {
        let (height, width) = event.get_size();

        true
    });*/

        Win {
            window: window,
            css_provider: css_provider,
            display: display,
            screen: screen,
            geometry: geometry,
            default_width: default_width,
            default_height: default_height,
            main_box: main_box,
        }
    }
}

fn percentage_to_value(distance: i32, percentage: i32) -> i32 {
    (distance * percentage) / 100
}

fn set_autoscroll(scrolled_window: &gtk::ScrolledWindow, child_window: &gtk::Box) {
    let adj = scrolled_window.get_vadjustment().unwrap();
    child_window.connect_size_allocate(move |_, _| {
        adj.set_value(adj.get_upper() - adj.get_page_size());
    });
}

fn main() {

    let user_token = &env::var("USER_TOKEN").expect("Couldn't get USER_TOKEN env variable");
    let contact_channel_id = &env::var("CONTACT_CHANNEL_ID").expect("Couldn't get CONTACT_CHANNEL_ID env variable");
    let contact_id = &env::var("CONTACT_ID").expect("Couldn't get CONTACT_ID env variable");

    let discord = Arc::new(Discord::from_user_token(user_token).expect("login failed"));
    let (mut connection, initial_state) = discord.connect().expect("connect failed");
    let (sender, receiver) = channel();

    let private_channels = initial_state.private_channels.clone();
    for channel in private_channels {
        if let discord::model::Channel::Private(channel) = channel {
            println!("{} -> {}", channel.recipient.name, channel.id);
        }
    }

    // GTK main thread
    {
        let discord = Arc::clone(&discord);
        thread::spawn(move || {
            if gtk::init().is_err() {
                println!("Failed to initialize GTK.");
                return;
            }

            let gui = Win::new();

            let chat_window = gtk::ScrolledWindow::new(None, None);
            chat_window.set_size_request(-1, percentage_to_value(gui.default_height, 90));
            let chat_box = gtk::Box::new(gtk::Orientation::Vertical, 10);

            set_autoscroll(&chat_window, &chat_box);

            let text_input = gtk::TextView::new();
            text_input.set_wrap_mode(gtk::WrapMode::WordChar);
            text_input.set_size_request(-1, percentage_to_value(gui.default_height, 10));

            chat_window.add(&chat_box);
            gui.main_box.add(&chat_window);
            gui.main_box.add(&text_input);
            gui.window.add(&gui.main_box);
            gui.window.show_all();

            let new_message = gtk::TextView::new();
            new_message.set_wrap_mode(gtk::WrapMode::WordChar);
            new_message.set_editable(false);
            new_message.set_cursor_visible(false);
            let context = new_message.get_style_context().unwrap();
            context.add_class("message");
            chat_box.add(&new_message);
            let new_message_buffer = new_message.get_buffer().unwrap();
            let tag_table = new_message_buffer.get_tag_table().unwrap();
            let gap_tag = gtk::TextTag::new("gap");
            gap_tag.set_property_pixels_above_lines(20);
            let nick_tag = gtk::TextTag::new("nick");
            nick_tag.set_property_left_margin(60);
            nick_tag.set_property_pixels_below_lines(5);
            nick_tag.set_property_weight(700);
            let message_tag = gtk::TextTag::new("message");
            message_tag.set_property_left_margin(60);
            message_tag.set_property_indent(0);
            let color_tag = gtk::TextTag::new("color");
            //color_tag.set_property_foreground_rgba(Some(&gdk::RGBA { red: 63f64, green: 81f64, blue: 181f64, alpha: 1f64}));
            color_tag.set_property_foreground(Some("#536dfe"));
            tag_table.add(&color_tag);
            tag_table.add(&gap_tag);
            tag_table.add(&nick_tag);
            tag_table.add(&message_tag);

            let state = gtk::Label::new("first time usage");
            let mut mstate = Arc::new(Mutex::new(String::from("")));
            // GTK keyboard events
            {
                //                let cloned_text_input = text_input.clone();
                let cloned_chat_box = chat_box.clone();
                let cloned_message = new_message.clone();
                let tag_table = tag_table.clone();
                let state = state.clone();
                let initial_state = initial_state.clone();
                text_input.connect_key_press_event(move |text_input, key| {
                    match key.get_keyval() {
                        key::Return => {
                            let buffer = text_input.get_buffer().unwrap();
                            let (start, end) = buffer.get_bounds();
                            let text = buffer.get_text(&start, &end, false).unwrap();
                            let _ = discord
                                .send_message(ChannelId(341316763868332034), &text, "", false);
                            let cloned_message_buffer = cloned_message.get_buffer().unwrap();
                            let mut iter = cloned_message_buffer.get_end_iter();

                            // Ensure new line
                            if iter.get_line_offset() != 0 {
                                cloned_message_buffer.insert(&mut iter, "\n");
                            }

                            iter = cloned_message_buffer.get_end_iter();

                            // Insert message
                            let gap_tag = tag_table.lookup("gap").unwrap();
                            let nick_tag = tag_table.lookup("nick").unwrap();
                            let message_tag = tag_table.lookup("message").unwrap();
                            let color_tag = tag_table.lookup("color").unwrap();

                            // Insert nickname with tag
                            let current_nickname = &state.get_label().unwrap();
                            let mut gap = false;
                            if current_nickname == "first time usage" || current_nickname != "" {
                                {
                                    //let mut cur: () = *mstate.lock().unwrap();
                                    //println!("Current nickname from: {:?}", cur);
                                    //cur.push_str("added");
                                    //println!("New nickname from: {:?}", cur);
                                }
                                state.set_label("");
                                gap = true;
                                let mut offset = iter.get_offset();
                                cloned_message_buffer
                                    .insert(&mut iter, &initial_state.user.username);
                                let mut start = cloned_message_buffer.get_iter_at_offset(offset);
                                cloned_message_buffer.remove_all_tags(&start, &iter);
                                cloned_message_buffer.apply_tag(&color_tag, &start, &iter);
                                cloned_message_buffer.apply_tag(&gap_tag, &start, &iter);
                                cloned_message_buffer.apply_tag(&nick_tag, &start, &iter);
                                cloned_message_buffer.insert(&mut iter, "\n");
                            }
                            // Insert message with tag
                            let offset = iter.get_offset();
                            cloned_message_buffer.insert(&mut iter, &text);
                            let start = cloned_message_buffer.get_iter_at_offset(offset);
                            cloned_message_buffer.remove_all_tags(&start, &iter);
                            cloned_message_buffer.apply_tag(&message_tag, &start, &iter);

                            cloned_chat_box.show_all();
                            gtk::timeout_add(0, move || {
                                buffer.set_text("");
                                gtk::Continue(false)
                            });
                        }
                        _ => (),
                    }
                    Inhibit(false)
                });
            }

            // GTK checks ASAP if new messages
            {
                let cloned_message = new_message.clone();
                let cloned_gui = gui.clone();
                let cloned_chat_box = chat_box.clone();
                let tag_table = tag_table.clone();
                let state = state.clone();
                gtk::timeout_add(500, move || match receiver.try_recv() {
                    Ok(message) => {
                        let message = message as discord::model::Message;
                        let cloned_message_buffer = cloned_message.get_buffer().unwrap();
                        let mut iter = cloned_message_buffer.get_end_iter();
                        //cloned_message_buffer.insert(&mut iter, "\n");
                        iter = cloned_message_buffer.get_end_iter();
                        // Ensure new line
                        if iter.get_line_offset() != 0 {
                            cloned_message_buffer.insert(&mut iter, "\n");
                        }

                        iter = cloned_message_buffer.get_end_iter();

                        // Insert message

                        let gap_tag = tag_table.lookup("gap").unwrap();
                        let nick_tag = tag_table.lookup("nick").unwrap();
                        let message_tag = tag_table.lookup("message").unwrap();
                        // Insert nickname with tag
                        let current_nickname = &state.get_label().unwrap();
                        let mut gap = false;
                        if current_nickname != &message.author.name {
                            state.set_label(&message.author.name);
                            gap = true;
                            let mut offset = iter.get_offset();
                            cloned_message_buffer.insert(&mut iter, &message.author.name);
                            let mut start = cloned_message_buffer.get_iter_at_offset(offset);
                            cloned_message_buffer.remove_all_tags(&start, &iter);
                            cloned_message_buffer.apply_tag(&gap_tag, &start, &iter);
                            cloned_message_buffer.apply_tag(&nick_tag, &start, &iter);
                            cloned_message_buffer.insert(&mut iter, "\n");
                        }
                        // Insert message with tag
                        let offset = iter.get_offset();
                        cloned_message_buffer.insert(&mut iter, &message.content);
                        let start = cloned_message_buffer.get_iter_at_offset(offset);
                        cloned_message_buffer.remove_all_tags(&start, &iter);
                        cloned_message_buffer.apply_tag(&message_tag, &start, &iter);
                        cloned_chat_box.show_all();
                        gtk::Continue(true)
                    }
                    Err(_) => gtk::Continue(true),
                });
            }

            gtk::main();
        });
    }

    // Discord API logic
    let discord = Arc::clone(&discord);
    let mut prev_state = ChannelId(0);
    loop {
        match connection.recv_event() {
            Ok(Event::MessageCreate(message)) => {
                if prev_state != message.channel_id {
                    prev_state = message.channel_id;
                }
                if message.author.discriminator == 4330 &&
                    message.channel_id == ChannelId(341316763868332034)
                {
                    sender.send(message.clone()).unwrap();
                }
            }
            Ok(_) => {}
            Err(discord::Error::Closed(code, body)) => {
                println!("Gateway closed on us with code {:?}: {}", code, body);
                break;
            }
            Err(err) => println!("Receive error: {:?}", err),
        }
    }
}
