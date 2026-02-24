//
// toplist.rs
// Copyright (C) 2022 gmg137 <gmg137 AT live.com>
// Distributed under terms of the GPL-3.0-or-later license.
//
use crate::{
    application::Action, model::ImageDownloadImpl, path::CACHE,
};
use adw::subclass::prelude::BinImpl;
use async_channel::Sender;
use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate, *};
use ncm_api::{SongList, TopList};
use once_cell::sync::OnceCell;
use std::cell::RefCell;

glib::wrapper! {
    pub struct TopListView(ObjectSubclass<imp::TopListView>)
        @extends adw::Bin, Widget,
        @implements Accessible, ConstraintTarget, Buildable;
}

impl Default for TopListView {
    fn default() -> Self {
        Self::new()
    }
}

impl TopListView {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn set_sender(&self, sender: Sender<Action>) {
        self.imp().sender.set(sender).unwrap();
    }

    pub fn init_toplist(&self, list: Vec<TopList>) {
        let imp = self.imp();
        let grid = imp.toplist_grid.get();
        let sender = imp.sender.get().unwrap();

        // Clear existing items
        while let Some(child) = grid.last_child() {
            grid.remove(&child);
        }

        for t in &list {
            let card = Self::create_toplist_card(t, sender);
            grid.insert(&card, -1);
        }
        imp.data.replace(list);
    }

    fn create_toplist_card(toplist: &TopList, sender: &Sender<Action>) -> Box {
        let vbox = Box::new(Orientation::Vertical, 0);

        let image = Image::builder()
            .pixel_size(140)
            .icon_name("image-missing-symbolic")
            .build();

        let frame = Frame::builder()
            .halign(Align::Center)
            .valign(Align::Center)
            .child(&image)
            .build();
        frame.add_css_class("cover-frame");

        let mut path = CACHE.clone();
        path.push(format!("{}-toplist.jpg", toplist.id));

        if !path.exists() {
            image.set_from_net(toplist.cover.to_owned(), path, (140, 140), sender);
        } else {
            image.set_from_file(Some(&path));
        }

        let label = Label::builder()
            .label(&toplist.name)
            .lines(2)
            .margin_start(10)
            .margin_end(10)
            .width_chars(1)
            .max_width_chars(1)
            .ellipsize(pango::EllipsizeMode::End)
            .wrap(true)
            .margin_top(6)
            .build();

        let update_label = Label::builder()
            .label(&toplist.update)
            .width_chars(1)
            .max_width_chars(1)
            .ellipsize(pango::EllipsizeMode::End)
            .margin_top(2)
            .css_classes(["dim-label"].map(String::from).to_vec())
            .build();

        vbox.append(&frame);
        vbox.append(&label);
        vbox.append(&update_label);

        vbox
    }
}

mod imp {

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/gitee/gmg137/NeteaseCloudMusicGtk4/gtk/toplist.ui")]
    pub struct TopListView {
        #[template_child]
        pub toplist_grid: TemplateChild<FlowBox>,

        pub data: RefCell<Vec<TopList>>,
        pub sender: OnceCell<Sender<Action>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TopListView {
        const NAME: &'static str = "TopListView";
        type Type = super::TopListView;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl TopListView {
        #[template_callback]
        fn toplist_card_activated_cb(&self, child: &FlowBoxChild) {
            let index = child.index() as usize;
            let data = self.data.borrow();
            if let Some(info) = data.get(index) {
                let sender = self.sender.get().unwrap();
                // Navigate to songlist detail page using existing ToSongListPage action
                let sl = SongList {
                    id: info.id,
                    name: info.name.clone(),
                    cover_img_url: info.cover.clone(),
                    author: String::new(),
                    creator_id: 0,
                };
                sender
                    .send_blocking(Action::ToSongListPage(sl))
                    .unwrap();
            }
        }
    }

    impl ObjectImpl for TopListView {
        fn constructed(&self) {
            self.parent_constructed();
        }
    }
    impl WidgetImpl for TopListView {}
    impl BinImpl for TopListView {}
}
