use gio::prelude::*;
use glib::{Bytes, ExitCode, clone};
use gtk::prelude::*;
use std::path::PathBuf;

struct App {
    window: Option<gtk::ApplicationWindow>,
    list_box: gtk::ListBox,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            list_box: gtk::ListBox::new(),
        }
    }

    fn build_ui(&mut self, app: &gtk::Application) {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("PrintHelper")
            .default_width(600)
            .default_height(500)
            .build();
        self.window = Some(window);
        let window = self.window.as_ref().unwrap();

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 6);
        vbox.set_margin_top(6);
        vbox.set_margin_bottom(6);
        vbox.set_margin_start(6);
        vbox.set_margin_end(6);

        // 工具栏
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let add_btn = gtk::Button::with_label("添加图片");
        let gen_btn = gtk::Button::with_label("生成 HTML");
        toolbar.append(&add_btn);
        toolbar.append(&gen_btn);
        vbox.append(&toolbar);

        // 图片列表
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        self.list_box = gtk::ListBox::new();
        self.list_box.set_selection_mode(gtk::SelectionMode::None);
        scrolled.set_child(Some(&self.list_box));
        vbox.append(&scrolled);

        window.set_child(Some(&vbox));

        self.setup_drop_target();
        self.connect_add_clicked(&add_btn);
        self.connect_gen_clicked(&gen_btn);
    }

    fn setup_drop_target(&self) {
        let drop_target = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::MOVE);
        let list_box = self.list_box.clone();

        drop_target.connect_drop(clone!(@strong list_box => move |_target, value, _x, y| {
            // 修复：value.get::<String>() 返回 Result，正确匹配 Ok(s)
            let src_idx: i32 = match value.get::<String>() {
                Ok(s) => s.parse().unwrap_or(-1),
                _ => -1,
            };
            if src_idx < 0 {
                return false;
            }

            let dest_idx = {
                let mut dest = None;
                if let Some(row_at_y) = list_box.row_at_y(y as i32) {
                    let idx = row_at_y.index();
                    if idx >= 0 {
                        dest = Some(idx);
                    }
                }
                dest.unwrap_or_else(|| {
                    let count = list_box_n_items(&list_box);
                    if count > 0 {
                        (count - 1) as i32
                    } else {
                        0
                    }
                })
            };

            if src_idx != dest_idx {
                if let Some(src_row) = list_box.row_at_index(src_idx) {
                    // 先移除再插入，避免 GTK 布局断言
                    list_box.remove(&src_row);
                    list_box.insert(&src_row, dest_idx);
                }
            }
            true
        }));

        self.list_box.add_controller(drop_target);
    }

    fn connect_add_clicked(&self, btn: &gtk::Button) {
        let window = self.window.clone().unwrap();
        let list_box = self.list_box.clone();

        btn.connect_clicked(move |_| {
            let dialog = gtk::FileChooserDialog::builder()
                .title("选择图片")
                .action(gtk::FileChooserAction::Open)
                .modal(true)
                .transient_for(&window)
                .select_multiple(true)
                .build();
            dialog.add_button("取消", gtk::ResponseType::Cancel);
            dialog.add_button("打开", gtk::ResponseType::Accept);

            let filter = gtk::FileFilter::new();
            filter.set_name(Some("图片文件"));
            // 添加所有支持的 MIME 类型
            filter.add_mime_type("image/png");
            filter.add_mime_type("image/jpeg");
            filter.add_mime_type("image/gif");
            filter.add_mime_type("image/bmp");
            filter.add_mime_type("image/webp");
            filter.add_mime_type("image/tiff");
            filter.add_mime_type("image/x-icon");
            filter.add_mime_type("image/vnd.microsoft.icon");
            filter.add_mime_type("image/x-portable-anymap");
            filter.add_mime_type("image/x-portable-bitmap");
            filter.add_mime_type("image/x-portable-graymap");
            filter.add_mime_type("image/x-portable-pixmap");
            filter.add_mime_type("image/x-dds");
            filter.add_mime_type("image/x-farbfeld");
            filter.add_mime_type("image/avif");
            // 常见扩展名模式（作为补充）
            filter.add_pattern("*.png");
            filter.add_pattern("*.jpg");
            filter.add_pattern("*.jpeg");
            filter.add_pattern("*.jpe");
            filter.add_pattern("*.jfif");
            filter.add_pattern("*.gif");
            filter.add_pattern("*.bmp");
            filter.add_pattern("*.dib");
            filter.add_pattern("*.webp");
            filter.add_pattern("*.tif");
            filter.add_pattern("*.tiff");
            filter.add_pattern("*.ico");
            filter.add_pattern("*.avif");
            filter.add_pattern("*.pbm");
            filter.add_pattern("*.pgm");
            filter.add_pattern("*.ppm");
            filter.add_pattern("*.pnm");
            filter.add_pattern("*.dds");
            filter.add_pattern("*.farbfeld");
            filter.add_pattern("*.hdr");

            let list_box = list_box.clone();
            dialog.connect_response(clone!(@strong list_box => move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    let files = dialog.files();
                    for i in 0..files.n_items() {
                        let file = files.item(i).unwrap().downcast::<gio::File>().unwrap();
                        if let Some(path) = file.path() {
                            if !path.exists() {
                                continue;
                            }
                            let list_box = list_box.clone();
                            glib::spawn_future_local(async move {
                                if let Some(texture) = generate_thumbnail(&path).await {
                                    let row = create_picture_row(&path, texture);
                                    connect_delete_button(&row, &list_box);
                                    connect_double_click(&row); // 新增：双击打开完整图片
                                    list_box.append(&row);
                                } else {
                                    eprintln!("无法加载图片: {}", path.display());
                                }
                            });
                        }
                    }
                }
                dialog.destroy();
            }));

            dialog.show();
        });
    }

    fn connect_gen_clicked(&self, btn: &gtk::Button) {
        let window = self.window.clone().unwrap();
        let list_box = self.list_box.clone();

        btn.connect_clicked(move |_| {
            let mut paths = Vec::new();
            let mut row = list_box.row_at_index(0);
            let mut idx = 0;
            while let Some(r) = row {
                if let Some(child) = r.child() {
                    if let Ok(hbox) = child.downcast::<gtk::Box>() {
                        // 隐藏的路径标签是第3个子元素（索引2）
                        if let Some(path_label) = hbox
                            .observe_children()
                            .item(2)
                            .and_then(|w| w.downcast::<gtk::Label>().ok())
                        {
                            let text = path_label.text();
                            if let Some(path_str) = text.strip_prefix("PATH:") {
                                paths.push(path_str.to_string());
                            }
                        }
                    }
                }
                idx += 1;
                row = list_box.row_at_index(idx);
            }

            if paths.is_empty() {
                return;
            }

            let html = generate_html(&paths);
            let dialog = gtk::FileChooserDialog::builder()
                .title("保存 HTML 文件")
                .action(gtk::FileChooserAction::Save)
                .modal(true)
                .transient_for(&window)
                .build();
            dialog.add_button("取消", gtk::ResponseType::Cancel);
            dialog.add_button("保存", gtk::ResponseType::Accept);
            dialog.set_current_name("print.html");

            let html_clone = html.clone();
            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            if let Err(e) = std::fs::write(path, &html_clone) {
                                eprintln!("写入文件失败: {}", e);
                            }
                        }
                    }
                }
                dialog.destroy();
            });

            dialog.show();
        });
    }
}

// ---------- 辅助函数 ----------
async fn generate_thumbnail(path: &PathBuf) -> Option<gdk::Texture> {
    let path = path.clone();
    let result = gio::spawn_blocking(move || {
        let img = match image::open(&path) {
            Ok(img) => img,
            Err(e) => {
                eprintln!("无法打开图片 {}: {}", path.display(), e);
                return None;
            }
        };
        let thumbnail = img.thumbnail(80, 80);
        let rgba = thumbnail.to_rgba8();
        let width = thumbnail.width();
        let height = thumbnail.height();

        let texture = gdk::MemoryTexture::new(
            width as i32,
            height as i32,
            gdk::MemoryFormat::R8g8b8a8Premultiplied,
            &Bytes::from(rgba.as_raw()),
            width as usize * 4,
        );
        Some(texture)
    })
    .await;

    result.ok().flatten().map(|tex| tex.into())
}

fn create_picture_row(path: &PathBuf, texture: gdk::Texture) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();

    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    hbox.set_margin_top(6);
    hbox.set_margin_bottom(6);
    hbox.set_margin_start(6);
    hbox.set_margin_end(6);

    // 缩略图
    let image = gtk::Picture::new();
    image.set_paintable(Some(&texture));
    image.set_size_request(80, 80);
    hbox.append(&image);

    // 文件名
    let name = path.file_name().unwrap().to_string_lossy();
    let label = gtk::Label::new(Some(&name));
    label.set_halign(gtk::Align::Start);
    label.set_hexpand(true);
    hbox.append(&label);

    // 隐藏的路径标签
    let path_label = gtk::Label::new(Some(&format!("PATH:{}", path.display())));
    path_label.set_visible(false);
    hbox.append(&path_label);

    // 删除按钮
    let delete_btn = gtk::Button::with_label("✕");
    delete_btn.set_valign(gtk::Align::Center);
    delete_btn.set_margin_end(6);
    hbox.append(&delete_btn);

    row.set_child(Some(&hbox));

    setup_drag_source(&row);
    row
}

/// 连接删除按钮
fn connect_delete_button(row: &gtk::ListBoxRow, list_box: &gtk::ListBox) {
    if let Some(child) = row.child() {
        if let Ok(hbox) = child.downcast::<gtk::Box>() {
            // 删除按钮是第4个子元素（索引3）
            if let Some(btn) = hbox
                .observe_children()
                .item(3)
                .and_then(|w| w.downcast::<gtk::Button>().ok())
            {
                let row_clone = row.clone();
                let list_box_clone = list_box.clone();
                btn.connect_clicked(move |_| {
                    list_box_clone.remove(&row_clone);
                });
            }
        }
    }
}

/// 新增：双击打开完整图片窗口
fn connect_double_click(row: &gtk::ListBoxRow) {
    let gesture = gtk::GestureClick::new();
    gesture.set_button(1);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

    gesture.connect_released(clone!(@strong row => move |_gesture, n_press, _x, _y| {
        if n_press == 2 {
            // 处理双击
            // 从 row 中提取路径，调用 open_fullsize_window
            if let Some(child) = row.child() {
                if let Ok(hbox) = child.downcast::<gtk::Box>() {
                    if let Some(path_label) = hbox.observe_children().item(2)
                        .and_then(|w| w.downcast::<gtk::Label>().ok())
                    {
                        let text = path_label.text();
                        if let Some(path_str) = text.strip_prefix("PATH:") {
                            let path = PathBuf::from(path_str);
                            open_fullsize_window(&path);
                        }
                    }
                }
            }
        }
    }));
    row.add_controller(gesture);
}

/// 打开新窗口显示原始图片
/// 打开新窗口显示原始图片
fn open_fullsize_window(path: &PathBuf) {
    let window = gtk::Window::builder()
        .title(format!(
            "完整图片 - {}",
            path.file_name().unwrap().to_string_lossy()
        ))
        .default_width(800)
        .default_height(600)
        .modal(true)
        .build();

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .build();

    let picture = gtk::Picture::new();
    picture.set_can_shrink(true);
    picture.set_keep_aspect_ratio(true);
    scrolled.set_child(Some(&picture));
    window.set_child(Some(&scrolled));

    // 克隆 window 和 picture，以便在异步任务中使用
    let window_clone = window.clone();
    let picture_clone = picture.clone();

    let path_clone = path.clone();
    glib::spawn_future_local(async move {
        let file = gio::File::for_path(&path_clone);
        if let Ok(texture) = gdk::Texture::from_file(&file) {
            picture_clone.set_paintable(Some(&texture));
        } else {
            eprintln!("无法加载完整图片: {}", path_clone.display());
            let label = gtk::Label::new(Some(&format!("无法加载图片:\n{}", path_clone.display())));
            window_clone.set_child(Some(&label));
        }
    });

    window.present();
}

fn setup_drag_source(row: &gtk::ListBoxRow) {
    let drag_source = gtk::DragSource::new();
    drag_source.set_actions(gdk::DragAction::MOVE);

    drag_source.connect_prepare(|source, _x, _y| {
        let widget = source.widget();
        if let Ok(row) = widget.downcast::<gtk::ListBoxRow>() {
            let idx = row.index();
            if idx >= 0 {
                let value = glib::Value::from(idx.to_string());
                let content = gdk::ContentProvider::for_value(&value);
                return Some(content);
            }
        }
        None
    });

    row.add_controller(drag_source);
}

/// 获取 ListBox 中的行数
fn list_box_n_items(list_box: &gtk::ListBox) -> u32 {
    let mut count = 0;
    while list_box.row_at_index(count as i32).is_some() {
        count += 1;
    }
    count
}

fn generate_html(paths: &[String]) -> String {
    let mut html = String::new();
    html.push_str(r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>PrintHelper</title>"#);
    html.push_str(
        r#"<style>
        body { margin: 0; padding: 0; background: white; }
        img {
            max-width: 100%;
            max-height: 100vh;
            display: block;
            margin: 0 auto;
            object-fit: contain;
            page-break-inside: avoid;
        }
    </style>"#,
    );
    html.push_str(r#"</head><body>"#);
    for (i, p) in paths.iter().enumerate() {
        let uri = p.replace(" ", "%20");
        if i == 0 {
            html.push_str(&format!(r#"<img src="file://{}">"#, uri));
        } else {
            html.push_str(&format!(
                r#"<div style="page-break-before: always;"></div><img src="file://{}">"#,
                uri
            ));
        }
    }
    html.push_str(r#"</body></html>"#);
    html
}

fn main() -> ExitCode {
    let app = gtk::Application::builder()
        .application_id("com.xy770.printhelper")
        .build();

    app.connect_activate(|app| {
        let mut app_ui = App::new();
        app_ui.build_ui(app);
        if let Some(window) = app_ui.window.as_ref() {
            window.present();
        }
    });

    app.run()
}
