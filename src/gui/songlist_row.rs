//
// songlist_row.rs
// Copyright (C) 2022 gmg137 <gmg137 AT live.com>
// Distributed under terms of the GPL-3.0-or-later license.
//
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, CompositeTemplate, *};

use crate::{application::Action, model::ImageDownloadImpl, path::CACHE};
use async_channel::Sender;
use gettextrs::gettext;
use glib::{clone, ParamSpec, ParamSpecBoolean, SendWeakRef, Value};
use ncm_api::{SongInfo, SongList};
use once_cell::sync::{Lazy, OnceCell};
use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

glib::wrapper! {
    pub struct SonglistRow(ObjectSubclass<imp::SonglistRow>)
        @extends Widget, ListBoxRow,
        @implements Accessible, Actionable, Buildable, ConstraintTarget;
}

impl SonglistRow {
    pub fn new(sender: Sender<Action>, si: &SongInfo) -> Self {
        let obj: Self = glib::Object::new();
        let imp = obj.imp();
        if imp.sender.get().is_none() {
            imp.sender.set(sender).unwrap();
        }
        obj.set_from_song_info(si);
        obj
    }

    pub fn set_from_song_info(&self, si: &SongInfo) {
        let imp = self.imp();
        imp.song_info.replace(Some(si.clone()));

        self.set_tooltip_text(Some(&si.name));
        self.set_name(&si.name);
        self.set_singer(&si.singer);
        self.set_album(&si.album);
        self.set_duration(si.duration);

        self.set_activatable(si.copyright.playable());

        // Load cover thumbnail
        if let Some(sender) = imp.sender.get() {
            let cover_image = imp.cover_image.get();
            let mut path = CACHE.clone();
            path.push(format!("{}-songlist.jpg", si.album_id));
            if path.exists() {
                cover_image.set_from_file(Some(&path));
            } else if !si.pic_url.is_empty() {
                cover_image.set_from_net(si.pic_url.to_owned(), path, (40, 40), sender);
            }
        }
    }

    pub fn not_ignore_grey(&self) -> bool {
        self.property("not_ignore_grey")
    }

    pub fn get_song_info(&self) -> Option<SongInfo> {
        self.imp().song_info.borrow().as_ref().cloned()
    }

    pub fn set_index(&self, n: usize) {
        let imp = self.imp();
        imp.index_label.set_label(&format!("{:02}", n));
    }

    pub fn switch_image(&self, visible: bool) {
        let imp = self.imp();
        imp.play_icon.set_visible(visible);
        imp.index_label.set_visible(!visible);
    }

    pub fn set_like_button_visible(&self, visible: bool) {
        let imp = self.imp();
        imp.like_button.set_visible(visible);
    }

    pub fn set_album_button_visible(&self, visible: bool) {
        let imp = self.imp();
        imp.album_button.set_visible(visible);
    }

    pub fn set_remove_button_visible(&self, visible: bool) {
        let imp = self.imp();
        imp.remove_button.set_visible(visible);
    }

    fn set_name(&self, label: &str) {
        let imp = self.imp();
        imp.title_label.set_label(label);
    }

    fn set_singer(&self, label: &str) {
        let imp = self.imp();
        if label.is_empty() {
            imp.artist_label.set_label(&gettext("Unknown artist"));
        } else {
            imp.artist_label.set_label(label);
        }
    }

    fn set_album(&self, label: &str) {
        let imp = self.imp();
        imp.album_label.set_label(label);
    }

    fn set_duration(&self, duration: u64) {
        let imp = self.imp();
        let label = format!("{:0>2}:{:0>2}", duration / 1000 / 60, duration / 1000 % 60);
        imp.duration_label.set_label(&label);
    }
}

#[gtk::template_callbacks]
impl SonglistRow {
    #[template_callback]
    fn on_click(&self, n_press: i32, _x: f64, _y: f64) {
        if n_press == 2 {
            self.emit_activate();
        }
    }

    #[template_callback]
    fn on_right_click(&self, _n_press: i32, x: f64, y: f64) {
        let imp = self.imp();

        // 动态构建菜单，根据 like 状态设置收藏文字
        let like = imp.like.get();
        let like_label = if like {
            "取消收藏"
        } else {
            "收藏"
        };

        let menu_model = gio::Menu::new();
        let section = gio::Menu::new();
        section.append(Some("播放"), Some("row.play-now"));
        section.append(Some("下一首播放"), Some("row.play-next"));
        section.append(Some(like_label), Some("row.toggle-like"));
        menu_model.append_section(None, &section);

        // 懒创建 PopoverMenu，clone 后释放 borrow 再 popup 防止重入 panic
        let menu = {
            let mut menu_ref = imp.context_menu.borrow_mut();
            if menu_ref.is_none() {
                let m = PopoverMenu::from_model(Some(&menu_model));
                m.set_parent(self.upcast_ref::<Widget>());
                m.set_has_arrow(false);
                m.set_can_focus(false);
                *menu_ref = Some(m);
            }
            menu_ref.as_ref().unwrap().clone()
        };

        menu.set_menu_model(Some(&menu_model));
        menu.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        menu.popup();
    }

    #[template_callback]
    fn like_button_clicked_cb(&self) {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap();
        let si = { imp.song_info.borrow().clone().unwrap() };
        let s_send = SendWeakRef::from(self.downgrade());
        let like = imp.like.get();
        sender
            .send_blocking(Action::LikeSong(
                si.id,
                !like,
                Some(Arc::new(move |_| {
                    if let Some(s) = s_send.upgrade() {
                        s.set_property("like", !like);
                    }
                })),
            ))
            .unwrap();
    }

    #[template_callback]
    fn album_button_clicked_cb(&self) {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap();
        let si = { imp.song_info.borrow().clone().unwrap() };
        if si.album_id != 0 {
            let songlist = SongList {
                id: si.album_id,
                name: si.album,
                cover_img_url: si.pic_url,
                author: String::new(),
            };
            sender.send_blocking(Action::ToAlbumPage(songlist)).unwrap();
        } else {
            sender
                .send_blocking(Action::AddToast(gettext("Album not found!")))
                .unwrap();
        }
    }

    #[template_callback]
    fn remove_button_clicked_cb(&self) {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap();
        let si = { imp.song_info.borrow().clone().unwrap() };
        sender
            .send_blocking(Action::RemoveFromPlayList(si))
            .unwrap();
    }
}

mod imp {

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/gitee/gmg137/NeteaseCloudMusicGtk4/gtk/songlist-row.ui")]
    pub struct SonglistRow {
        #[template_child]
        pub play_icon: TemplateChild<Image>,
        #[template_child]
        pub index_label: TemplateChild<Label>,
        #[template_child]
        pub cover_image: TemplateChild<Image>,
        #[template_child]
        pub title_label: TemplateChild<Label>,
        #[template_child]
        pub artist_label: TemplateChild<Label>,
        #[template_child]
        pub album_label: TemplateChild<Label>,
        #[template_child]
        pub duration_label: TemplateChild<Label>,
        #[template_child]
        pub like_button: TemplateChild<Button>,
        #[template_child]
        pub album_button: TemplateChild<Button>,
        #[template_child]
        pub remove_button: TemplateChild<Button>,

        pub context_menu: RefCell<Option<PopoverMenu>>,
        pub sender: OnceCell<Sender<Action>>,
        pub song_info: RefCell<Option<SongInfo>>,

        pub like: Cell<bool>,
        pub not_ignore_grey: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SonglistRow {
        const NAME: &'static str = "SonglistRow";
        type Type = super::SonglistRow;
        type ParentType = ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SonglistRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            obj.bind_property("like", &self.like_button.get(), "icon_name")
                .transform_to(|_, v: bool| {
                    Some(
                        (if v {
                            "starred-symbolic"
                        } else {
                            "non-starred-symbolic"
                        })
                        .to_string(),
                    )
                })
                .build();

            // 注册右键菜单 actions
            let action_group = gio::SimpleActionGroup::new();

            let play_now = gio::SimpleAction::new("play-now", None);
            play_now.connect_activate(clone!(
                #[weak]
                obj,
                move |_, _| {
                    let imp = obj.imp();
                    if let (Some(sender), Some(si)) =
                        (imp.sender.get(), imp.song_info.borrow().clone())
                    {
                        sender.send_blocking(Action::PlayNow(si)).unwrap();
                    }
                }
            ));
            action_group.add_action(&play_now);

            let play_next = gio::SimpleAction::new("play-next", None);
            play_next.connect_activate(clone!(
                #[weak]
                obj,
                move |_, _| {
                    let imp = obj.imp();
                    if let (Some(sender), Some(si)) =
                        (imp.sender.get(), imp.song_info.borrow().clone())
                    {
                        sender.send_blocking(Action::PlayNext(si)).unwrap();
                    }
                }
            ));
            action_group.add_action(&play_next);

            let toggle_like = gio::SimpleAction::new("toggle-like", None);
            toggle_like.connect_activate(clone!(
                #[weak]
                obj,
                move |_, _| {
                    let imp = obj.imp();
                    if let Some(sender) = imp.sender.get() {
                        let si = imp.song_info.borrow().clone().unwrap();
                        let s_send = glib::SendWeakRef::from(obj.downgrade());
                        let like = imp.like.get();
                        sender
                            .send_blocking(Action::LikeSong(
                                si.id,
                                !like,
                                Some(Arc::new(move |_| {
                                    if let Some(s) = s_send.upgrade() {
                                        s.set_property("like", !like);
                                    }
                                })),
                            ))
                            .unwrap();
                    }
                }
            ));
            action_group.add_action(&toggle_like);

            obj.insert_action_group("row", Some(&action_group));
        }

        fn dispose(&self) {
            if let Some(menu) = self.context_menu.borrow().as_ref() {
                menu.unparent();
            }
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecBoolean::builder("like").build(),
                    ParamSpecBoolean::builder("not-ignore-grey").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "like" => {
                    let like = value.get().expect("The value needs to be of type `bool`.");
                    self.like.replace(like);
                }
                "not-ignore-grey" => {
                    let val: bool = value.get().unwrap();
                    self.not_ignore_grey.replace(val);
                }
                n => unimplemented!("{}", n),
            }
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "like" => self.like.get().to_value(),
                "not-ignore-grey" => self.not_ignore_grey.get().to_value(),
                n => unimplemented!("{}", n),
            }
        }
    }
    impl WidgetImpl for SonglistRow {}
    impl ListBoxRowImpl for SonglistRow {}
}
