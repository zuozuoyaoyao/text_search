// Re-export MainWindow from the generated Slint code
slint::include_modules!();

use crate::config::Config;
use crate::database::Database;
use anyhow::Result;
use slint::{ComponentHandle, ModelRc, SharedString, StandardListViewItem, VecModel};
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct SearchResultItem {
    pub name: String,
    pub path: String,
    pub context: String,
    pub is_start: bool,
    pub is_end: bool,
}


impl SearchResultItem {
    pub fn from_database_result(result: crate::database::SearchResult) -> Self {
        Self {
            name: result.name,
            path: result.path,
            context: result.context,
            is_start: result.is_start,
            is_end: result.is_end,
        }
    }
}

pub struct GuiApp {
    pub app: MainWindow,
    db: Arc<Mutex<Database>>,
    config: Arc<Mutex<Config>>,
}

impl GuiApp {
    pub fn new(db: Arc<Mutex<Database>>, config: Arc<Mutex<Config>>) -> Result<Self> {
        let app = MainWindow::new().map_err(|e| anyhow::anyhow!("Failed to create GUI: {}", e))?;

        let gui = Self { app, db, config };

        // Setup callbacks
        gui.setup_callbacks();

        Ok(gui)
    }
    // 转换函数 - 将数据转换为表格模型

    fn setup_callbacks(&self) {
        let app_weak = self.app.as_weak();
        let app_weak_search = app_weak.clone();
        let app_weak_settings = app_weak.clone();
        let app_weak_file = app_weak.clone();
        let db = Arc::clone(&self.db);
        let config = Arc::clone(&self.config);

        self.app.on_search(move |keyword, context_length| {
            let app = app_weak_search.upgrade().unwrap();
            let keyword = keyword.trim();

            if keyword.is_empty() {
                return;
            }

            // Set status message
            app.set_status_message(SharedString::from(format!("Searching for: {}...", keyword)));

            let db = db.lock().unwrap();
            match db.search(keyword, context_length as usize) {
                Ok(results) => {
                    let table_vec: Vec<ModelRc<StandardListViewItem>> = vec![];
                    let table_rows2 = Rc::new(VecModel::from(table_vec));
                    // Convert results to SearchResultRow for ListView
                    let mut table_rows: Vec<SearchResultRow> = Vec::new();
                    for (idx, result) in results.iter().enumerate() {

                        // Extract file type
                        let file_type = Path::new(&result.name)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_uppercase();
                        let file_name: &str = &result.name;
                        let file_type_display = if file_type.is_empty() { "-" } else { &file_type };

                        // Format context
                        let context = if !result.is_start {
                            format!("...{}", result.context)
                        } else if !result.is_end {
                            format!("{}...", result.context)
                        } else {
                            result.context.clone()
                        };

                        // Create row
                        // table_rows.push(SearchResultRow {
                        //     index: (idx + 1) as i32,
                        //     file_name: item.name.into(),
                        //     file_type: file_type_display.into(),
                        //     context: context.into(),
                        // });
                        let row = ModelRc::from([
                           StandardListViewItem::from((idx +1).to_string().as_str()),
                           StandardListViewItem::from(file_name),
                           StandardListViewItem::from(file_type_display),
                           StandardListViewItem::from(context.as_str()),
                        ]);
                        table_rows2.push(row);
                    }

                    // Create VecModel and update GUI
                    app.set_dynamic_rows(table_rows2.into());

                    // Set status message
                    let status = if results.is_empty() {
                        "No results found".to_string()
                    } else {
                        format!("Found {} result(s)", results.len())
                    };
                    app.set_status_message(SharedString::from(status));

                    // Update SQL query display
                    app.set_sql_query(SharedString::from(format!(
                        "SELECT name, abs_path, content FROM file WHERE content LIKE '%{}%'",
                        keyword
                    )));

                    tracing::info!("Search completed: found {} result(s)", results.len());
                }
                Err(e) => {
                    tracing::error!("Search error: {}", e);
                    app.set_status_message(SharedString::from(format!("Search error: {}", e)));
                }
            }
        });

        self.app.on_save_settings(move || {
            let app = app_weak_settings.upgrade().unwrap();
            let config = config.lock().unwrap();
            if let Err(e) = config.save() {
                tracing::error!("Failed to save config: {}", e);
                app.set_status_message(SharedString::from("Failed to save config"));
            } else {
                tracing::info!("Settings saved");
                app.set_status_message(SharedString::from("Settings saved successfully"));
            }
        });

        self.app.on_open_file(move |path| {
            let app = app_weak_file.upgrade().unwrap();
            tracing::info!("Open file request: {}", path);
            // For now, just log the path. In the future, we could:
            // - Open the file with default application
            // - Show file location
            // - Copy file path to clipboard
            app.set_status_message(SharedString::from(format!("File: {}", path)));
        });
    }

    pub fn show(&self) {
        self.app.run().unwrap();
    }

    pub fn show_window(&self) {
        self.app.show().unwrap();
    }

    pub fn hide_window(&self) {
        self.app.hide().unwrap();
    }

    pub fn is_visible(&self) -> bool {
        true
    }
}
