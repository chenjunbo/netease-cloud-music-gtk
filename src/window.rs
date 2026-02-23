use crate::{
    application::{Action, NeteaseCloudMusicGtk4Application},
    audio::MprisController,
    gui::*,
    model::*,
    ncmapi::NcmClient,
};
use adw::{ColorScheme, StyleManager, Toast};
use async_channel::Sender;
use gettextrs::gettext;
use gio::{Settings, SimpleAction};
use glib::{
    ParamSpec, ParamSpecEnum, ParamSpecObject, ParamSpecUInt64, Value, clone, source::Priority,
};
use gtk::{
    CompositeTemplate,
    gio::{self, SettingsBindFlags},
    glib,
    graphene,
};
use log::*;
use ncm_api::{BannersInfo, LoginInfo, SongInfo, SongList, TopList};
use once_cell::sync::{Lazy, OnceCell};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    sync::Arc,
};

mod imp {

    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/com/gitee/gmg137/NeteaseCloudMusicGtk4/gtk/window.ui")]
    pub struct NeteaseCloudMusicGtk4Window {
        #[template_child]
        pub header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub gbox: TemplateChild<Box>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub base_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub back_button: TemplateChild<Button>,
        #[template_child]
        pub search_button: TemplateChild<ToggleButton>,
        #[template_child]
        pub search_bar: TemplateChild<SearchBar>,
        #[template_child]
        pub search_entry: TemplateChild<SearchEntry>,
        #[template_child]
        pub search_menu: TemplateChild<MenuButton>,
        #[template_child]
        pub primary_menu_button: TemplateChild<MenuButton>,
        #[template_child]
        pub label_title: TemplateChild<Label>,
        #[template_child]
        pub user_button: TemplateChild<MenuButton>,
        #[template_child]
        pub player_revealer: TemplateChild<Revealer>,
        #[template_child]
        pub player_controls: TemplateChild<PlayerControls>,
        #[template_child]
        pub content_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub toplist: TemplateChild<TopListView>,
        #[template_child]
        pub discover: TemplateChild<Discover>,

        // Sidebar widgets
        #[template_child]
        pub nav_listbox: TemplateChild<ListBox>,
        #[template_child]
        pub nav_discover: TemplateChild<ListBoxRow>,
        #[template_child]
        pub nav_toplist: TemplateChild<ListBoxRow>,
        #[template_child]
        pub my_section_label: TemplateChild<Label>,
        #[template_child]
        pub my_listbox: TemplateChild<ListBox>,
        #[template_child]
        pub created_playlists_expander: TemplateChild<gtk::Expander>,
        #[template_child]
        pub created_playlists_listbox: TemplateChild<ListBox>,
        #[template_child]
        pub collected_playlists_expander: TemplateChild<gtk::Expander>,
        #[template_child]
        pub collected_playlists_listbox: TemplateChild<ListBox>,

        // Playlist drawer
        #[template_child]
        pub content_overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub playlist_drawer_revealer: TemplateChild<Revealer>,
        #[template_child]
        pub drawer_songs_list: TemplateChild<SongListView>,
        #[template_child]
        pub drawer_count_label: TemplateChild<Label>,

        // Lyrics overlay
        #[template_child]
        pub lyrics_overlay_revealer: TemplateChild<Revealer>,
        #[template_child]
        pub lyrics_overlay_title: TemplateChild<Label>,
        #[template_child]
        pub lyrics_overlay_artist: TemplateChild<Label>,
        #[template_child]
        pub lyrics_scroll: TemplateChild<ScrolledWindow>,
        #[template_child]
        pub lyrics_lines_box: TemplateChild<gtk::Box>,
        pub overlay_lyrics: RefCell<Vec<(u64, String)>>,
        pub overlay_labels: RefCell<Vec<Label>>,

        pub playlist_lyrics_page: OnceCell<PlayListLyricsPage>,

        pub user_menus: OnceCell<UserMenus>,
        pub popover_menu: OnceCell<PopoverMenu>,
        pub settings: OnceCell<Settings>,
        pub sender: OnceCell<Sender<Action>>,
        pub page_stack: OnceCell<PageStack>,

        // Sidebar playlist data
        pub created_playlists: RefCell<Vec<SongList>>,
        pub collected_playlists: RefCell<Vec<SongList>>,

        search_type: Cell<SearchType>,
        toast: RefCell<Option<Toast>>,
        user_info: RefCell<UserInfo>,
    }

    impl NeteaseCloudMusicGtk4Window {
        pub fn user_like_song_contains(&self, id: &u64) -> bool {
            self.user_info.borrow().like_songs.contains(id)
        }
        pub fn user_like_song_add(&self, id: u64) {
            self.user_info.borrow_mut().like_songs.insert(id);
        }
        pub fn user_like_song_remove(&self, id: &u64) {
            self.user_info.borrow_mut().like_songs.remove(id);
        }
        pub fn clear_user_info(&self) {
            self.user_info.take();
        }
        pub fn set_nickname(&self, nickname: String) {
            self.user_info.borrow_mut().nickname = nickname;
        }
        pub fn get_nickname(&self) -> String {
            self.user_info.borrow().nickname.clone()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NeteaseCloudMusicGtk4Window {
        const NAME: &'static str = "NeteaseCloudMusicGtk4Window";
        type Type = super::NeteaseCloudMusicGtk4Window;
        type ParentType = ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for NeteaseCloudMusicGtk4Window {
        fn constructed(&self) {
            let obj = self.obj();
            self.parent_constructed();

            load_css();

            self.page_stack
                .set(PageStack::new(self.base_stack.get()))
                .unwrap();

            self.playlist_lyrics_page
                .set(PlayListLyricsPage::new())
                .unwrap();

            self.toast.replace(Some(Toast::new("")));

            // Select discover row by default
            self.nav_listbox.select_row(Some(&*self.nav_discover));

            obj.setup_settings();
            obj.bind_settings();
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecEnum::builder::<SearchType>("search-type")
                        .explicit_notify()
                        .build(),
                    ParamSpecObject::builder::<Toast>("toast").build(),
                    ParamSpecUInt64::builder("uid").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "toast" => {
                    let toast = value.get().unwrap();
                    self.toast.replace(toast);
                }
                "search-type" => {
                    let input_type = value
                        .get()
                        .expect("The value needs to be of type `SearchType`.");
                    self.search_type.replace(input_type);
                }
                "uid" => {
                    let uid = value.get().unwrap();
                    self.user_info.borrow_mut().uid = uid;
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "toast" => self.toast.borrow().to_value(),
                "search-type" => self.search_type.get().to_value(),
                "uid" => self.user_info.borrow().uid.to_value(),
                _ => unimplemented!(),
            }
        }
    }
    impl WidgetImpl for NeteaseCloudMusicGtk4Window {}
    impl WindowImpl for NeteaseCloudMusicGtk4Window {
        fn close_request(&self) -> glib::Propagation {
            // Save playlist state before window closes
            self.player_controls.get().save_playlist_state();
            self.parent_close_request()
        }
    }
    impl ApplicationWindowImpl for NeteaseCloudMusicGtk4Window {}

    fn load_css() {
        let provider = gtk::CssProvider::new();
        provider.load_from_resource(
            "/com/gitee/gmg137/NeteaseCloudMusicGtk4/themes/style.css",
        );
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("Could not connect to a display."),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

glib::wrapper! {
    pub struct NeteaseCloudMusicGtk4Window(ObjectSubclass<imp::NeteaseCloudMusicGtk4Window>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl NeteaseCloudMusicGtk4Window {
    pub fn new<P: glib::object::IsA<gtk::Application>>(
        application: &P,
        sender: Sender<Action>,
    ) -> Self {
        let window: NeteaseCloudMusicGtk4Window = glib::Object::builder()
            .property("application", application)
            .build();

        window.imp().sender.set(sender).unwrap();
        window.setup_widget();
        window.setup_action();
        window.init_page_data();
        window
    }

    fn setup_settings(&self) {
        let settings = Settings::new(crate::APP_ID);
        self.imp()
            .settings
            .set(settings)
            .expect("Could not set `Settings`.");
    }

    pub fn settings(&self) -> &Settings {
        self.imp().settings.get().expect("Could not get settings.")
    }

    fn setup_action(&self) {
        let imp = self.imp();
        let sender_ = imp.sender.get().unwrap().clone();
        // 监测用户菜单弹出
        let popover = imp.popover_menu.get().unwrap();
        let sender = sender_.clone();
        popover.connect_child_notify(move |_| {
            sender.send_blocking(Action::TryUpdateQrCode).unwrap();
        });
        let sender = sender_.clone();
        popover.connect_show(move |_| {
            sender.send_blocking(Action::TryUpdateQrCode).unwrap();
        });

        // 绑定设置与主题
        let action_style = self.settings().create_action("style-variant");
        self.add_action(&action_style);

        // 绑定搜索按钮和搜索栏
        let search_button = imp.search_button.get();
        let search_entry = imp.search_entry.get();

        // 设置搜索动作
        let action_search = SimpleAction::new("search-button", None);
        action_search.connect_activate(clone!(
            #[weak]
            search_button,
            move |_, _| {
                search_button.emit_clicked();
            }
        ));
        self.add_action(&action_search);

        let search_bar = imp.search_bar.get();
        search_bar.connect_search_mode_enabled_notify(clone!(
            #[weak]
            search_entry,
            move |bar| {
                if bar.is_search_mode() {
                    // 清空搜索框
                    search_entry.set_text("");
                    // 使搜索框获取输入焦点
                    search_entry.grab_focus();
                }
            }
        ));

        // 设置返回键功能
        let action_back = SimpleAction::new("back-button", None);
        self.add_action(&action_back);

        let sender = sender_;
        action_back.connect_activate(move |_, _| {
            sender.send_blocking(Action::PageBack).unwrap();
        });
    }

    fn bind_settings(&self) {
        let style = StyleManager::default();
        self.settings()
            .bind("style-variant", &style, "color-scheme")
            .mapping(|themes, _| {
                let themes = themes
                    .get::<String>()
                    .expect("The variant needs to be of type `String`.");
                let scheme = match themes.as_str() {
                    "system" => ColorScheme::Default,
                    "light" => ColorScheme::ForceLight,
                    "dark" => ColorScheme::ForceDark,
                    _ => ColorScheme::Default,
                };
                Some(scheme.to_value())
            })
            .build();

        self.settings()
            .bind("exit-switch", self, "hide-on-close")
            .flags(SettingsBindFlags::DEFAULT)
            .build();
    }

    fn setup_widget(&self) {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap();
        let primary_menu_button = imp.primary_menu_button.get();
        let popover = primary_menu_button.popover().unwrap();
        let popover = popover.downcast::<gtk::PopoverMenu>().unwrap();
        let theme = crate::gui::ThemeSelector::new();
        popover.add_child(&theme, "theme");

        let user_menus = UserMenus::new(sender.clone());

        let user_button = imp.user_button.get();
        let popover = user_button.popover().unwrap();
        let popover = popover.downcast::<PopoverMenu>().unwrap();
        popover.add_child(&user_menus.qrbox, "user_popover");

        imp.user_menus.set(user_menus).unwrap();
        imp.popover_menu.set(popover).unwrap();
    }

    pub fn get_uid(&self) -> u64 {
        self.property::<u64>("uid")
    }

    pub fn set_uid(&self, val: u64) {
        self.set_property("uid", val);
    }

    pub fn is_logined(&self) -> bool {
        self.get_uid() != 0u64
    }

    pub fn set_nickname(&self, nickname: String) {
        self.imp().set_nickname(nickname);
    }

    pub fn get_nickname(&self) -> String {
        self.imp().get_nickname()
    }

    pub fn logout(&self) {
        self.imp().clear_user_info();
    }

    pub fn get_song_likes(&self, sis: &[SongInfo]) -> Vec<bool> {
        sis.iter()
            .map(|si| self.imp().user_like_song_contains(&si.id))
            .collect()
    }

    pub fn set_like_song(&self, id: u64, val: bool) {
        let imp = self.imp();
        if let Some(song) = imp.player_controls.get().get_current_song() {
            if song.id == id {
                imp.player_controls.get().set_property("like", val);
            }
        }

        if val {
            imp.user_like_song_add(id);
        } else {
            imp.user_like_song_remove(&id);
        }
    }

    pub fn set_user_like_songs(&self, song_ids: &[u64]) {
        song_ids
            .iter()
            .for_each(|id| self.imp().user_like_song_add(id.to_owned()));
    }

    pub fn set_user_qrimage(&self, path: PathBuf) {
        let user_menus = self.imp().user_menus.get().unwrap();
        user_menus.set_qrimage(path);
    }

    pub fn set_user_qrimage_timeout(&self) {
        let user_menus = self.imp().user_menus.get().unwrap();
        user_menus.set_qrimage_timeout();
    }

    pub fn is_user_menu_active(&self, menu: UserMenuChild) -> bool {
        self.imp().user_menus.get().unwrap().is_menu_active(menu)
    }

    pub fn switch_user_menu_to_phone(&self) {
        let popover = self.imp().popover_menu.get().unwrap();
        let user_menus = self.imp().user_menus.get().unwrap();
        user_menus.switch_menu(UserMenuChild::Phone, popover);
    }

    pub fn switch_user_menu_to_qr(&self) {
        let popover = self.imp().popover_menu.get().unwrap();
        let user_menus = self.imp().user_menus.get().unwrap();
        user_menus.switch_menu(UserMenuChild::Qr, popover);
    }

    pub fn switch_user_menu_to_user(&self, login_info: LoginInfo, _menu: UserMenuChild) {
        let popover = self.imp().popover_menu.get().unwrap();
        let user_menus = self.imp().user_menus.get().unwrap();
        user_menus.switch_menu(UserMenuChild::User, popover);
        if login_info.vip_type == 0 {
            user_menus.set_user_name(login_info.nickname);
        } else {
            user_menus.set_user_name(format!("👑{}", login_info.nickname));
        }
    }

    pub fn set_avatar(&self, url: String, path: PathBuf) {
        self.imp()
            .user_menus
            .get()
            .unwrap()
            .set_user_avatar(url, path);
    }

    pub fn add_toast(&self, mes: String) {
        let pre = self.property::<Toast>("toast");

        let toast = Toast::builder()
            .title(glib::markup_escape_text(&mes))
            .priority(adw::ToastPriority::High)
            .build();
        self.set_property("toast", &toast);
        self.imp().toast_overlay.add_toast(toast);

        crate::MAINCONTEXT.spawn_local_with_priority(Priority::DEFAULT_IDLE, async move {
            glib::timeout_future(std::time::Duration::from_millis(500)).await;
            pre.dismiss();
        });
    }

    pub fn add_carousel(&self, banner: BannersInfo) {
        let discover = self.imp().discover.get();
        discover.add_carousel(banner);
    }

    pub fn setup_top_picks(&self, song_list: Vec<SongList>) {
        let discover = self.imp().discover.get();
        discover.setup_top_picks(song_list);
    }

    pub fn setup_new_albums(&self, song_list: Vec<SongList>) {
        let discover = self.imp().discover.get();
        discover.setup_new_albums(song_list);
    }

    pub fn add_play(&self, song_info: SongInfo) {
        let player_controls = self.imp().player_controls.get();
        player_controls.add_song(song_info);
    }

    pub fn insert_next(&self, song_info: SongInfo) {
        let player_controls = self.imp().player_controls.get();
        player_controls.insert_next(song_info);
    }

    pub fn remove_from_playlist(&self, song_info: SongInfo) {
        let imp = self.imp();
        let player_controls = imp.player_controls.get();
        player_controls.remove_song(song_info);

        let sis = player_controls.get_list();
        let si = player_controls.get_current_song().unwrap_or(SongInfo {
            id: 0,
            name: String::new(),
            singer: String::new(),
            album: String::new(),
            album_id: 0,
            pic_url: String::new(),
            duration: 0,
            song_url: String::new(),
            copyright: ncm_api::SongCopyright::Unknown,
        });

        self.init_playlist_lyrics_page(sis, si.to_owned());

        if si.id == 0 {
            let sender = imp.sender.get().unwrap();
            sender.send_blocking(Action::PageBack).unwrap();
        }
        self.update_button_sensitivity();

        // Refresh drawer if open
        if imp.playlist_drawer_revealer.reveals_child() {
            self.update_drawer_playlist();
        }
    }

    pub fn add_playlist(&self, sis: Vec<SongInfo>, is_play: bool) {
        let player_controls = self.imp().player_controls.get();
        player_controls.add_list(sis);
        let sender = self.imp().sender.get().unwrap();
        if is_play {
            sender.send_blocking(Action::PlayListStart).unwrap();
        }
    }

    pub fn add_playlist_at(&self, sis: Vec<SongInfo>, index: usize) {
        let player_controls = self.imp().player_controls.get();
        player_controls.add_list(sis);
        player_controls.set_playlist_position(index);
        let sender = self.imp().sender.get().unwrap();
        sender.send_blocking(Action::PlayListStart).unwrap();
    }

    pub fn playlist_start(&self) {
        let sender = self.imp().sender.get().unwrap();
        let player_controls = self.imp().player_controls.get();
        if let Some(song_info) = player_controls.get_current_song() {
            sender.send_blocking(Action::Play(song_info)).unwrap();
            return;
        }
        sender
            .send_blocking(Action::AddToast(gettext("No playable songs found！")))
            .unwrap();
    }

    pub fn play_next(&self) {
        let player_controls = self.imp().player_controls.get();
        player_controls.next_song();
    }

    pub fn play(&self, song_info: SongInfo) {
        let player_controls = self.imp().player_controls.get();
        player_controls.set_property("like", self.imp().user_like_song_contains(&song_info.id));
        player_controls.play(song_info);
    }

    pub fn init_page_data(&self) {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap();

        // 初始化播放栏
        let player_controls = imp.player_controls.get();
        player_controls.set_sender(sender.clone());

        // 初始化发现页
        let discover = imp.discover.get();
        discover.set_sender(sender.clone());
        discover.init_page();

        // 初始化榜单
        sender.send_blocking(Action::GetToplist).unwrap();
        let toplist = imp.toplist.get();
        toplist.set_sender(sender.clone());

        // 初始化播放列表页
        let playlist_lyrics_page = imp.playlist_lyrics_page.get().unwrap();
        playlist_lyrics_page.set_sender(sender.clone());

        // 初始化播放列表抽屉
        let drawer_songs_list = imp.drawer_songs_list.get();
        drawer_songs_list.set_sender(sender.clone());
        drawer_songs_list.set_property("no-act-album", true);

        // 点击抽屉外区域自动关闭（挂在 gbox 上，覆盖侧边栏+内容区）
        let gesture = gtk::GestureClick::new();
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
        let revealer = imp.playlist_drawer_revealer.get();
        let gbox = imp.gbox.get();
        gesture.connect_pressed(clone!(
            #[weak]
            revealer,
            #[weak]
            gbox,
            move |gesture, _, x, y| {
                if revealer.reveals_child() {
                    if let Some(target) = gbox.pick(x, y, gtk::PickFlags::DEFAULT) {
                        if !target.is_ancestor(&revealer)
                            && target != *revealer.upcast_ref::<Widget>()
                        {
                            revealer.set_reveal_child(false);
                            gesture.set_state(gtk::EventSequenceState::Claimed);
                        }
                    }
                }
            }
        ));
        imp.gbox.add_controller(gesture);

        let page_stack = imp.page_stack.get().unwrap();
        page_stack.set_transition_type(StackTransitionType::Crossfade);
        page_stack.set_transition_duration(100);
    }

    pub fn init_toplist(&self, list: Vec<TopList>) {
        let toplist = self.imp().toplist.get();
        toplist.init_toplist(list);
    }

    // page routing
    fn page_widget_switch(&self, need_back: bool) {
        let imp = self.imp();
        let back_button = imp.back_button.get();
        back_button.set_visible(need_back);
    }

    pub fn page_set_info(&self, title: &str) {
        let imp = self.imp();
        let label_title = imp.label_title.get();
        label_title.set_label(title);
    }

    // same name will clear old page
    pub fn page_new_with_name(
        &self,
        name: &str,
        page: &impl glib::object::IsA<Widget>,
        title: &str,
    ) {
        let imp = self.imp();
        let stack = imp.page_stack.get().unwrap();
        let stack_page = stack.new_page_with_name(page, name);
        stack_page.set_title(title);
        self.page_set_info(title);
        self.page_widget_switch(true);
    }

    pub fn page_new(&self, page: &impl glib::object::IsA<Widget>, title: &str, name: &str) {
        let imp = self.imp();
        let stack = imp.page_stack.get().unwrap();
        if stack.len() > 1 {
            let top_page = stack.top_page();
            if top_page.title().unwrap() == title {
                if let Some(n) = top_page.name() {
                    if n == name {
                        return;
                    }
                } else {
                    return;
                }
            }
        }
        let stack_page = stack.new_page(page);
        stack_page.set_title(title);
        stack_page.set_name(name);
        self.page_set_info(title);
        self.page_widget_switch(true);
    }

    pub fn page_back(&self) -> Option<Widget> {
        let imp = self.imp();
        let stack = imp.page_stack.get().unwrap();

        stack.back_page();

        if stack.len() > 1 {
            let top_page = stack.top_page();
            self.page_set_info(top_page.title().unwrap().to_string().as_str());
            self.page_widget_switch(true);
        } else {
            self.page_widget_switch(false);
            // Restore title based on current visible page
            let content_stack = imp.content_stack.get();
            if let Some(name) = content_stack.visible_child_name() {
                match name.as_str() {
                    "discover_page" => self.page_set_info(&gettext("Netease Cloud Music")),
                    "toplist_page" => self.page_set_info(&gettext("Toplist")),
                    _ => self.page_set_info(&gettext("Netease Cloud Music")),
                }
            }
        }
        None
    }

    pub fn update_button_sensitivity(&self) {
        self.imp().player_controls.get().update_button_sensitivity();
    }

    pub fn toggle_playlist_drawer(&self) {
        let imp = self.imp();
        let revealer = imp.playlist_drawer_revealer.get();
        let is_open = revealer.reveals_child();
        if !is_open {
            self.update_drawer_playlist();
        }
        revealer.set_reveal_child(!is_open);
    }

    pub fn update_drawer_playlist(&self) {
        let imp = self.imp();
        let player_controls = imp.player_controls.get();
        let sis = player_controls.get_list();
        let current_song = player_controls.get_current_song();

        let drawer_songs_list = imp.drawer_songs_list.get();
        drawer_songs_list.clear_list();

        let likes = self.get_song_likes(&sis);

        imp.drawer_count_label.set_label(&format!("{}", sis.len()));

        drawer_songs_list.init_new_list(&sis, &likes);

        // Highlight current song, set play/pause icon, and scroll to center
        if let Some(current) = current_song {
            if let Some(idx) = sis.iter().position(|s| s.id == current.id) {
                drawer_songs_list.mark_new_row_playing(idx as i32, false);
                let icon = if player_controls.is_playing() {
                    "media-playback-pause-symbolic"
                } else {
                    "media-playback-start-symbolic"
                };
                if let Some(row) = drawer_songs_list.list_box().row_at_index(idx as i32) {
                    let row = row.downcast::<SonglistRow>().unwrap();
                    row.set_cover_play_icon_name(icon);
                }
                // 延迟滚动，等 revealer 动画（200ms）结束后再定位
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(250),
                    clone!(
                        #[weak]
                        drawer_songs_list,
                        move || {
                            drawer_songs_list.scroll_to_row(idx as i32);
                        }
                    ),
                );
            }
        }
    }

    pub fn update_drawer_play_state(&self) {
        let imp = self.imp();
        if !imp.playlist_drawer_revealer.reveals_child() {
            return;
        }
        let icon = if imp.player_controls.get().is_playing() {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        };
        let listbox = imp.drawer_songs_list.list_box();
        let mut idx = 0;
        while let Some(row) = listbox.row_at_index(idx) {
            let row = row.downcast::<SonglistRow>().unwrap();
            if row.has_css_class("playing-row") {
                row.set_cover_play_icon_name(icon);
                break;
            }
            idx += 1;
        }
    }

    pub fn clear_playlist(&self) {
        let imp = self.imp();
        imp.player_controls.get().clear_all();
        imp.playlist_drawer_revealer.set_reveal_child(false);
    }

    pub fn save_playlist_state(&self) {
        self.imp().player_controls.get().save_playlist_state();
    }

    pub fn restore_playlist_state(&self) {
        self.imp().player_controls.get().restore_playlist_state();
    }

    pub fn persist_volume(&self, value: f64) {
        let imp = self.imp();
        imp.player_controls.persist_volume(value);
    }

    pub fn page_cur_playlist_lyrics_page(&self) -> bool {
        let imp = self.imp();
        let page = imp.playlist_lyrics_page.get().unwrap();
        let cur = &imp.page_stack.get().unwrap().top_page().child();
        cur == page
    }

    pub fn init_picks_songlist(&self) -> SearchSongListPage {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap().clone();
        let page = SearchSongListPage::new();
        page.set_sender(sender);
        page.init_page("全部歌单", SearchType::TopPicks);
        page
    }

    pub fn init_all_albums(&self) -> SearchSongListPage {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap().clone();
        let page = SearchSongListPage::new();
        page.set_sender(sender);
        page.init_page("全部新碟", SearchType::AllAlbums);
        page
    }

    pub fn init_search_song_page(&self, text: &str, search_type: SearchType) -> SearchSongPage {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap().clone();
        let page = SearchSongPage::new();
        page.set_sender(sender);
        page.init_page(text, search_type);
        page
    }

    pub fn init_search_songlist_page(
        &self,
        text: &str,
        search_type: SearchType,
    ) -> SearchSongListPage {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap().clone();
        let page = SearchSongListPage::new();
        page.set_sender(sender);
        page.init_page(text, search_type);
        page
    }
    pub fn init_search_singer_page(&self, text: &str) -> SearchSingerPage {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap().clone();
        let page = SearchSingerPage::new();
        page.set_sender(sender);
        page.init_page(text.to_string());
        page
    }

    pub fn init_songlist_page(&self, songlist: &SongList, is_album: bool) -> SonglistPage {
        let sender = self.imp().sender.get().unwrap().clone();
        let page = SonglistPage::new();
        page.set_sender(sender);
        page.init_songlist_info(songlist, is_album, self.is_logined());
        page
    }

    pub fn update_search_song_page(&self, page: SearchSongPage, sis: Vec<SongInfo>) {
        page.update_songs(&sis, &self.get_song_likes(&sis));
    }

    pub fn update_songlist_page(&self, page: SonglistPage, detail: &SongListDetail) {
        page.init_songlist(detail, &self.get_song_likes(detail.sis()));
    }

    // Sidebar visibility for login/logout
    pub fn show_my_sidebar(&self) {
        let imp = self.imp();
        imp.my_section_label.set_visible(true);
        imp.my_listbox.set_visible(true);
        imp.created_playlists_expander.set_visible(true);
        // collected expander visibility is controlled by init_sidebar_playlists_split
    }

    pub fn hide_my_sidebar(&self) {
        let imp = self.imp();
        imp.my_section_label.set_visible(false);
        imp.my_listbox.set_visible(false);
        imp.created_playlists_expander.set_visible(false);
        imp.collected_playlists_expander.set_visible(false);
        // Clear playlist data
        imp.created_playlists.borrow_mut().clear();
        imp.collected_playlists.borrow_mut().clear();
        // Clear listbox children
        Self::clear_listbox(&imp.created_playlists_listbox);
        Self::clear_listbox(&imp.collected_playlists_listbox);
    }

    fn clear_listbox(listbox: &ListBox) {
        while let Some(child) = listbox.first_child() {
            listbox.remove(&child);
        }
    }

    pub fn init_sidebar_playlists(&self, sls: Vec<SongList>) {
        let created: Vec<SongList> = sls.into_iter().skip(1).collect();
        self.init_sidebar_playlists_split(created, Vec::new());
    }

    pub fn init_sidebar_playlists_split(&self, created: Vec<SongList>, collected: Vec<SongList>) {
        let imp = self.imp();

        // Clear old data
        Self::clear_listbox(&imp.created_playlists_listbox);
        Self::clear_listbox(&imp.collected_playlists_listbox);

        // Update expander labels with count
        imp.created_playlists_expander
            .set_label(Some(&format!("创建的歌单 ({})", created.len())));
        imp.collected_playlists_expander
            .set_label(Some(&format!("收藏的歌单 ({})", collected.len())));

        // Only show collected expander if there are collected playlists
        imp.collected_playlists_expander.set_visible(!collected.is_empty());

        let sender = imp.sender.get().unwrap();
        for sl in &created {
            let row = Self::create_playlist_row(sl, sender);
            imp.created_playlists_listbox.append(&row);
        }

        for sl in &collected {
            let row = Self::create_playlist_row(sl, sender);
            imp.collected_playlists_listbox.append(&row);
        }

        imp.created_playlists.replace(created);
        imp.collected_playlists.replace(collected);
    }

    fn create_playlist_row(sl: &SongList, sender: &Sender<Action>) -> ListBoxRow {
        let label = Label::builder()
            .label(&sl.name)
            .halign(gtk::Align::Start)
            .ellipsize(pango::EllipsizeMode::End)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        let image = Image::builder()
            .pixel_size(24)
            .icon_name("audio-x-generic-symbolic")
            .build();

        let mut path = crate::path::CACHE.clone();
        path.push(format!("{}-songlist.jpg", sl.id));
        if !path.exists() {
            image.set_from_net(sl.cover_img_url.to_owned(), path, (140, 140), sender);
        } else {
            image.set_from_file(Some(&path));
        }

        let hbox = Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_start(8)
            .margin_end(8)
            .margin_top(2)
            .margin_bottom(2)
            .build();
        hbox.append(&image);
        hbox.append(&label);

        let row = ListBoxRow::new();
        row.set_child(Some(&hbox));
        row
    }

    pub fn init_playlist_lyrics_page(&self, sis: Vec<SongInfo>, si: SongInfo) {
        let imp = self.imp();
        let page = imp.playlist_lyrics_page.get().unwrap();
        page.init_page(&sis, si, &self.get_song_likes(&sis));

        self.page_new(page, &gettext("Play List&Lyrics"), "Play List&Lyrics");
    }

    /// 更新歌词内容，不调整位置
    pub fn update_lyrics(&self, lrc: Vec<(u64, String)>) {
        let imp = self.imp();
        let page = imp.playlist_lyrics_page.get().unwrap();
        page.update_lyrics(lrc);
    }

    /// 强行更新歌词区文字，用于显示歌词加载提示
    pub fn update_lyrics_text(&self, text: &str) {
        let imp = self.imp();
        let page = imp.playlist_lyrics_page.get().unwrap();
        page.update_lyrics_text(text);
    }

    // 更新歌词高亮位置
    pub fn update_lyrics_timestamp(&self, time: u64) {
        let imp = self.imp();
        let page = imp.playlist_lyrics_page.get().unwrap();
        if self.page_cur_playlist_lyrics_page() {
            page.update_lyrics_highlight(time);
        }
    }

    // ===== Lyrics Overlay =====

    pub fn show_lyrics_overlay(&self, si: &ncm_api::SongInfo, lyrics: Vec<(u64, String)>) {
        let imp = self.imp();

        // Set title and artist
        imp.lyrics_overlay_title.set_label(&si.name);
        imp.lyrics_overlay_artist.set_label(&si.singer);

        // Clear old labels
        let lines_box = imp.lyrics_lines_box.get();
        while let Some(child) = lines_box.first_child() {
            lines_box.remove(&child);
        }

        let mut labels = Vec::new();

        if lyrics.is_empty() {
            let label = Label::new(Some("暂无歌词"));
            label.add_css_class("lyrics-line");
            lines_box.append(&label);
            labels.push(label);
        } else {
            for (_, text) in &lyrics {
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                let label = Label::new(Some(text));
                label.set_halign(gtk::Align::Center);
                label.add_css_class("lyrics-line");
                lines_box.append(&label);
                labels.push(label);
            }
        }

        imp.overlay_lyrics.replace(lyrics);
        imp.overlay_labels.replace(labels);

        // Show the overlay
        imp.lyrics_overlay_revealer.set_reveal_child(true);
    }

    pub fn hide_lyrics_overlay(&self) {
        self.imp().lyrics_overlay_revealer.set_reveal_child(false);
    }

    pub fn is_lyrics_overlay_visible(&self) -> bool {
        self.imp().lyrics_overlay_revealer.reveals_child()
    }

    pub fn update_lyrics_overlay_data(&self, lyrics: Vec<(u64, String)>) {
        if !self.is_lyrics_overlay_visible() {
            return;
        }
        let imp = self.imp();

        // Rebuild labels with new lyrics
        let lines_box = imp.lyrics_lines_box.get();
        while let Some(child) = lines_box.first_child() {
            lines_box.remove(&child);
        }

        let mut labels = Vec::new();
        if lyrics.is_empty() {
            let label = Label::new(Some("暂无歌词"));
            label.add_css_class("lyrics-line");
            lines_box.append(&label);
            labels.push(label);
        } else {
            for (_, text) in &lyrics {
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                let label = Label::new(Some(text));
                label.set_halign(gtk::Align::Center);
                label.add_css_class("lyrics-line");
                lines_box.append(&label);
                labels.push(label);
            }
        }

        // Also update title/artist from current song
        if let Some(si) = imp.player_controls.get().get_current_song() {
            imp.lyrics_overlay_title.set_label(&si.name);
            imp.lyrics_overlay_artist.set_label(&si.singer);
        }

        imp.overlay_lyrics.replace(lyrics);
        imp.overlay_labels.replace(labels);
    }

    pub fn update_lyrics_overlay_highlight(&self, time: u64) {
        if !self.is_lyrics_overlay_visible() {
            return;
        }
        let imp = self.imp();
        let lyrics = imp.overlay_lyrics.borrow().clone();
        let labels = imp.overlay_labels.borrow();

        if lyrics.is_empty() || labels.is_empty() {
            return;
        }

        let playing = crate::gui::get_playing_indexes(lyrics.clone(), time);

        // Build a set of label indices that correspond to non-empty lyrics lines
        // (we skipped empty lines when creating labels)
        let mut lyric_to_label: Vec<usize> = Vec::new();
        let mut label_idx = 0;
        for (_, text) in &lyrics {
            if text.trim().is_empty() {
                lyric_to_label.push(usize::MAX); // no label for empty lines
            } else {
                lyric_to_label.push(label_idx);
                label_idx += 1;
            }
        }

        // Remove active class from all
        for label in labels.iter() {
            label.remove_css_class("lyrics-line-active");
            if !label.has_css_class("lyrics-line") {
                label.add_css_class("lyrics-line");
            }
        }

        if let Some((start, end)) = playing {
            for i in start..=end {
                if i < lyric_to_label.len() {
                    let li = lyric_to_label[i];
                    if li < labels.len() {
                        labels[li].remove_css_class("lyrics-line");
                        labels[li].add_css_class("lyrics-line-active");

                        // Scroll to center the active label
                        if i == start {
                            let scroll = imp.lyrics_scroll.get();
                            if let Some(adj) = Some(scroll.vadjustment()) {
                                let label_widget = &labels[li];
                                // Use compute_point to find label position relative to scroll
                                if let Some(point) = label_widget.compute_point(
                                    &*imp.lyrics_lines_box,
                                    &graphene::Point::new(0.0, 0.0),
                                ) {
                                    let label_y = point.y() as f64;
                                    let label_h = label_widget.height() as f64;
                                    let scroll_h = scroll.height() as f64;
                                    let target = label_y + label_h / 2.0 - scroll_h / 2.0;
                                    adj.set_value(target.max(0.0));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn update_playlist_status(&self, index: usize) {
        let imp = self.imp();
        let page = imp.playlist_lyrics_page.get().unwrap();
        if self.page_cur_playlist_lyrics_page() {
            page.switch_row(index as i32);
        }
    }

    pub fn set_song_url(&self, si: SongInfo) {
        self.imp().player_controls.get().set_song_url(si);
    }
    pub fn gst_duration_changed(&self, sec: u64) {
        self.imp().player_controls.get().gst_duration_changed(sec);
    }
    pub fn gst_state_changed(&self, state: gstreamer_play::PlayState) {
        self.imp().player_controls.get().gst_state_changed(state);
        self.update_drawer_play_state();
    }
    pub fn gst_volume_changed(&self, volume: f64) {
        self.imp().player_controls.get().gst_volume_changed(volume);
    }
    pub fn gst_cache_download_complete(&self, loc: String) {
        self.imp()
            .player_controls
            .get()
            .gst_cache_download_complete(loc);
    }
    pub fn scale_seek_update(&self, sec: u64) {
        self.imp().player_controls.get().scale_seek_update(sec);
    }
    pub fn scale_value_update(&self) {
        self.imp().player_controls.get().scale_value_update();
    }
    pub fn init_mpris(&self, mpris: MprisController) {
        self.imp().player_controls.get().init_mpris(mpris);
    }

    // Helper: unselect all sidebar listboxes except the given one
    fn unselect_other_listboxes(&self, except: &ListBox) {
        let imp = self.imp();
        let listboxes: [&ListBox; 4] = [
            &imp.nav_listbox,
            &imp.my_listbox,
            &imp.created_playlists_listbox,
            &imp.collected_playlists_listbox,
        ];
        for lb in &listboxes {
            if *lb != except {
                lb.unselect_all();
            }
        }
    }

    pub async fn action_search(
        &self,
        ncmapi: NcmClient,
        text: String,
        search_type: SearchType,
        offset: u16,
        limit: u16,
    ) -> Option<SearchResult> {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap().clone();
        let window = self;

        let res = match search_type {
            SearchType::Song => ncmapi
                .client
                .search_song(text, offset, limit)
                .await
                .map(|res| {
                    debug!("搜索歌曲：{:?}", res);
                    let likes = window.get_song_likes(&res);
                    SearchResult::Songs(res, likes)
                }),
            SearchType::Singer => {
                ncmapi
                    .client
                    .search_singer(text, offset, limit)
                    .await
                    .map(|res| {
                        debug!("搜索歌手：{:?}", res);
                        SearchResult::Singers(res)
                    })
            }
            SearchType::Album => ncmapi
                .client
                .search_album(text, offset, limit)
                .await
                .map(|res| {
                    debug!("搜索专辑：{:?}", res);
                    SearchResult::SongLists(res)
                }),
            SearchType::Lyrics => {
                ncmapi
                    .client
                    .search_lyrics(text, offset, limit)
                    .await
                    .map(|res| {
                        debug!("搜索歌词：{:?}", res);
                        let likes = window.get_song_likes(&res);
                        SearchResult::Songs(res, likes)
                    })
            }
            SearchType::SongList => ncmapi
                .client
                .search_songlist(text, offset, limit)
                .await
                .map(|res| {
                    debug!("搜索歌单：{:?}", res);
                    SearchResult::SongLists(res)
                }),
            SearchType::TopPicks => ncmapi
                .client
                .top_song_list("全部", "hot", offset, limit)
                .await
                .map(|res| {
                    debug!("获取歌单：{:?}", res);
                    SearchResult::SongLists(res)
                }),
            SearchType::AllAlbums => {
                ncmapi
                    .client
                    .new_albums("ALL", offset, limit)
                    .await
                    .map(|res| {
                        debug!("获取专辑：{:?}", res);
                        SearchResult::SongLists(res)
                    })
            }
            SearchType::Radio => ncmapi
                .client
                .user_radio_sublist(offset, limit)
                .await
                .map(|res| {
                    debug!("获取电台：{:?}", res);
                    SearchResult::SongLists(res)
                }),
            SearchType::LikeAlbums => ncmapi.client.album_sublist(offset, limit).await.map(|res| {
                debug!("获取收藏的专辑：{:?}", res);
                SearchResult::SongLists(res)
            }),
            SearchType::LikeSongList => {
                let uid = window.get_uid();
                ncmapi
                    .client
                    .user_song_list(uid, offset, limit)
                    .await
                    .map(|res| {
                        debug!("获取收藏的歌单：{:?}", res);
                        SearchResult::SongLists(res)
                    })
            }
            _ => Err(anyhow::anyhow!("")),
        };
        if let Err(err) = &res {
            error!("{:?}", err);
            sender
                .send_blocking(Action::AddToast(gettext(
                    "Request for interface failed, please try again!",
                )))
                .unwrap();
        }
        res.ok()
    }
}

#[gtk::template_callbacks]
impl NeteaseCloudMusicGtk4Window {
    #[template_callback]
    fn nav_row_activated_cb(&self, row: &ListBoxRow) {
        let imp = self.imp();
        self.unselect_other_listboxes(&imp.nav_listbox);

        let content_stack = imp.content_stack.get();
        let page_stack = imp.page_stack.get().unwrap();

        // Pop back to root if we have detail pages on the stack
        while page_stack.len() > 1 {
            page_stack.back_page();
        }
        self.page_widget_switch(false);

        if row == &*imp.nav_discover {
            content_stack.set_visible_child_name("discover_page");
            self.page_set_info(&gettext("Netease Cloud Music"));
        } else if row == &*imp.nav_toplist {
            content_stack.set_visible_child_name("toplist_page");
            self.page_set_info(&gettext("Toplist"));
        }
    }

    #[template_callback]
    fn my_row_activated_cb(&self, row: &ListBoxRow) {
        let imp = self.imp();
        self.unselect_other_listboxes(&imp.my_listbox);

        let sender = imp.sender.get().unwrap();
        let index = row.index();
        match index {
            0 => sender.send_blocking(Action::ToMyPageDailyRec).unwrap(),
            1 => sender.send_blocking(Action::ToMyPageHeartbeat).unwrap(),
            2 => sender.send_blocking(Action::ToMyPageRadio).unwrap(),
            3 => sender.send_blocking(Action::ToMyPageCloudDisk).unwrap(),
            4 => sender.send_blocking(Action::ToMyPageAlbums).unwrap(),
            _ => {}
        }
    }

    #[template_callback]
    fn created_playlist_row_activated_cb(&self, row: &ListBoxRow) {
        let imp = self.imp();
        self.unselect_other_listboxes(&imp.created_playlists_listbox);

        let index = row.index() as usize;
        let playlists = imp.created_playlists.borrow();
        if let Some(sl) = playlists.get(index) {
            let sender = imp.sender.get().unwrap();
            sender
                .send_blocking(Action::ToSongListPage(sl.clone()))
                .unwrap();
        }
    }

    #[template_callback]
    fn collected_playlist_row_activated_cb(&self, row: &ListBoxRow) {
        let imp = self.imp();
        self.unselect_other_listboxes(&imp.collected_playlists_listbox);

        let index = row.index() as usize;
        let playlists = imp.collected_playlists.borrow();
        if let Some(sl) = playlists.get(index) {
            let sender = imp.sender.get().unwrap();
            sender
                .send_blocking(Action::ToSongListPage(sl.clone()))
                .unwrap();
        }
    }

    #[template_callback]
    fn search_song_cb(&self, check: CheckButton) {
        let menu = self.imp().search_menu.get();
        menu.set_label(&check.label().unwrap());
        self.set_property("search-type", SearchType::Song);
    }

    #[template_callback]
    fn search_singer_cb(&self, check: CheckButton) {
        let menu = self.imp().search_menu.get();
        menu.set_label(&check.label().unwrap());
        self.set_property("search-type", SearchType::Singer);
    }

    #[template_callback]
    fn search_album_cb(&self, check: CheckButton) {
        let menu = self.imp().search_menu.get();
        menu.set_label(&check.label().unwrap());
        self.set_property("search-type", SearchType::Album);
    }

    #[template_callback]
    fn search_lyrics_cb(&self, check: CheckButton) {
        let menu = self.imp().search_menu.get();
        menu.set_label(&check.label().unwrap());
        self.set_property("search-type", SearchType::Lyrics);
    }

    #[template_callback]
    fn search_songlist_cb(&self, check: CheckButton) {
        let menu = self.imp().search_menu.get();
        menu.set_label(&check.label().unwrap());
        self.set_property("search-type", SearchType::SongList);
    }

    #[template_callback]
    fn drawer_clear_clicked_cb(&self) {
        let sender = self.imp().sender.get().unwrap();
        sender.send_blocking(Action::ClearPlaylist).unwrap();
    }

    #[template_callback]
    fn lyrics_overlay_close_cb(&self) {
        self.hide_lyrics_overlay();
    }

    #[template_callback]
    fn search_entry_cb(&self, entry: SearchEntry) {
        let imp = self.imp();
        let sender = imp.sender.get().unwrap();
        let text = entry.text().to_string();
        imp.label_title.set_label(&text);
        imp.back_button.set_visible(true);

        let search_type = self.property::<SearchType>("search-type");

        let page = match search_type {
            SearchType::Lyrics | SearchType::Song => {
                let page = self.init_search_song_page(&text, search_type);
                Some(page.upcast::<Widget>())
            }
            SearchType::Singer => {
                let page = self.init_search_singer_page(&text);
                Some(page.upcast::<Widget>())
            }
            SearchType::Album | SearchType::SongList => {
                let page = self.init_search_songlist_page(&text, search_type);
                Some(page.upcast::<Widget>())
            }
            _ => None,
        };
        if let Some(page) = page {
            self.page_new_with_name("search", &page, text.as_str());
            let page = glib::SendWeakRef::from(page.downgrade());
            sender
                .send_blocking(Action::Search(
                    text,
                    search_type,
                    0,
                    50,
                    Arc::new(move |res| {
                        if let Some(page) = page.upgrade() {
                            match res {
                                SearchResult::Songs(sis, likes) => {
                                    page.downcast::<SearchSongPage>()
                                        .unwrap()
                                        .update_songs(&sis, &likes);
                                }
                                SearchResult::Singers(sgs) => {
                                    page.downcast::<SearchSingerPage>()
                                        .unwrap()
                                        .update_singer(sgs);
                                }
                                SearchResult::SongLists(sls) => {
                                    page.downcast::<SearchSongListPage>()
                                        .unwrap()
                                        .update_songlist(&sls);
                                }
                            };
                        }
                    }),
                ))
                .unwrap();
        }
    }
}

impl Default for NeteaseCloudMusicGtk4Window {
    fn default() -> Self {
        NeteaseCloudMusicGtk4Application::default()
            .active_window()
            .unwrap()
            .downcast()
            .unwrap()
    }
}
