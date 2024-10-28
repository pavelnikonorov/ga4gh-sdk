use clap::{arg, Command};
use ga4gh_sdk::clients::tes::models::ListTasksParams;
use ga4gh_sdk::clients::tes::models::TesTask;
use ga4gh_sdk::clients::tes::{Task, TES};
use ga4gh_sdk::utils::configuration::Configuration;
use ga4gh_sdk::utils::test_utils::ensure_funnel_running;
use ga4gh_sdk::utils::transport::Transport;
use std::error::Error;
use std::fs;
use std::path::Path;
use url::Url;
use log::{debug, error};

use ga4gh_sdk::clients::tes::models::TesListTasksResponse;

/// # Examples
///
/// To run the `create` command:
///
/// ```sh
/// ga4gh-cli tes create '{
///     "name": "Hello world",
///     "inputs": [{
///         "url": "s3://funnel-bucket/hello.txt",
///         "path": "/inputs/hello.txt"
///     }],
///     "outputs": [{
///         "url": "s3://funnel-bucket/output.txt",
///         "path": "/outputs/stdout"
///     }],
///     "executors": [{
///         "image": "alpine",
///         "command": ["cat", "/inputs/hello.txt"],
///         "stdout": "/outputs/stdout"
///     }]
/// }'
/// ```
///
/// Or:
///
/// ```sh
/// ga4gh-cli tes create ./tests/sample.tes
/// ```
///
/// To run the `list` command:
///
/// ```sh
/// ga4gh-cli tes list --name_prefix None --state None --tag_key None --tag_value None --page_size None --page_token None --view FULL'
/// ```
/// OR
/// Parameters with None values can be avoided, like:
/// ```sh
/// ga4gh-cli tes list --view FULL
/// ```
///
/// ASSUME, cqgk5lj93m0311u6p530 is the id of a task created before
/// To run the `get` command:
///
/// ```sh
/// ga4gh-cli tes get cqgk5lj93m0311u6p530 BASIC
/// ```
///
/// To run the `status` command:
///
/// ```sh
/// ga4gh-cli tes status cqgk5lj93m0311u6p530      
/// ```
///
///
/// To run the `cancel` command:
///
/// ```sh
/// ga4gh-cli tes cancel cqgk5lj93m0311u6p530      
/// ```

use ga4gh_sdk::clients::tes::models::TesState;

fn tes_state_to_str(state: &Option<TesState>) -> &str {
    match state {
        Some(TesState::Unknown) => "Unknown",
        Some(TesState::Queued) => "Queued",
        Some(TesState::Initializing) => "Initializing",
        Some(TesState::Running) => "Running",
        Some(TesState::Paused) => "Paused",
        Some(TesState::Complete) => "Complete",
        Some(TesState::ExecutorError) => "Executor Error",
        Some(TesState::SystemError) => "System Error",
        Some(TesState::Canceled) => "Canceled",
        Some(TesState::Canceling) => "Canceling",
        Some(TesState::Preempted) => "Preempted",
        None => "None",
    }
}

fn format_task(task: &TesTask) -> String {
    format!(
        "{:<25} {:<15}\n",
        task.id.as_deref().unwrap_or("None"),
        tes_state_to_str(&task.state)
    )
}

fn format_tasks_response(response: &TesListTasksResponse) -> String {
    let mut table = String::new();
    let headers = format!("{:<25} {:<15}\n", "TASK ID", "State");
    table.push_str(&headers);
    for task in &response.tasks {
        table.push_str(&format_task(task));
    }
    table
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let cmd = Command::new("cli")
        .bin_name("cli")
        .version("0.1.0")
        .about("CLI to manage tasks")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("tes")
                .about("TES subcommands")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("create")
                        .about("Create a task")
                        .arg(arg!(<TASK_FILE> "The task file to create"))
                        // .arg(arg!(--url <URL> "The URL for the task"))
                        .arg_required_else_help(true),
                )
                .subcommand(
                    Command::new("list")
                    .about("list all tasks")
                    .arg(arg!(-n --name_prefix [NAME_PREFIX] "The name prefix to filter tasks"))
                    .arg(arg!(-s --state [STATE] "The state to filter tasks"))
                    .arg(arg!(-k --tag_key [TAG_KEY] "The tag key to filter tasks"))
                    .arg(arg!(-v --tag_value [TAG_VALUE] "The tag value to filter tasks"))
                    .arg(arg!(-p --page_size [PAGE_SIZE] "The page size for pagination"))
                    .arg(arg!(-t --page_token [PAGE_TOKEN] "The page token for pagination"))
                    .arg(arg!(-w --view [VIEW] "The view for the tasks"))
                )
                .subcommand(
                    Command::new("get")
                        .about("get task data")
                        .arg(arg!(<id> "The id of the task which should be returned"))
                        .arg(arg!(<view> "The view in which the task should be returned"))
                        .arg_required_else_help(true),
                )
                .subcommand(
                    Command::new("status")
                        .about("get status of the task")
                        .arg(arg!(<id> "The id of the task which should be returned"))
                        .arg_required_else_help(true),
                )
                .subcommand(
                    Command::new("cancel")
                        .about("cancel the task")
                        .arg(arg!(<id> "The id of the task which should be cancel"))
                        .arg_required_else_help(true),
                ),
        );

    let matches = cmd.clone().get_matches();

    match matches.subcommand() {
        Some(("tes", sub)) => {
            if let Some(("create", sub)) = sub.subcommand() {
                let task_file = sub.value_of("TASK_FILE").unwrap();
                // let url = sub.value_of("url").unwrap();
                let path = Path::new(task_file);
                if !path.exists() {
                    error!("File does not exist: {:?}", path);
                }
                let task_json = match fs::read_to_string(path) {
                    Ok(contents) => contents,
                    Err(e) => {
                        error!("Failed to read file: {}", e);
                        task_file.to_string()
                    }
                };
                let testask: TesTask = serde_json::from_str(&task_json)
                    .map_err(|e| format!("Failed to parse JSON: {}", e))?;
                let mut config = Configuration::default();
                // let mut config = load_configuration();
                let funnel_url = ensure_funnel_running().await;
                let funnel_url = url::Url::parse(&funnel_url).expect("Invalid URL");
                config.set_base_path(funnel_url);
                match TES::new(&config).await {
                    Ok(tes) => {
                        let task = tes.create(testask).await;
                        println!("{:?}", task);
                    }
                    Err(e) => {
                        error!("Error creating TES instance: {:?}", e);
                        return Err(e);
                    }
                };
            }
            if let Some(("list", sub)) = sub.subcommand() {
                debug!("list subcommand");
                let name_prefix = sub.value_of("name_prefix").map(|s| s.to_string());
                let state = sub.value_of("state").map(|s| serde_json::from_str(s).expect("Invalid state"));
                let _tag_key = sub.value_of("tag_key").map(|s| s.to_string());
                let _tag_value = sub.value_of("tag_value").map(|s| s.to_string());
                let page_size = sub.value_of("page_size").map(|s| s.parse().expect("Invalid page_size"));
                let page_token = sub.value_of("page_token").map(|s| s.to_string());
                let view = sub.value_of("view").map(|s| s.to_string());

                let parameters = ListTasksParams {
                    name_prefix,
                    state,
                    tag_key: None, // Example does not cover parsing Vec<String>
                    tag_value: None, // Example does not cover parsing Vec<String>
                    page_size,
                    page_token,
                    view,
                };

                debug!("parameters are: {:?}", parameters);
                let mut config = Configuration::default();
                
                let funnel_url = ensure_funnel_running().await;
                let funnel_url = url::Url::parse(&funnel_url).expect("Invalid URL");
                config.set_base_path(funnel_url);
                
                match TES::new(&config).await {
                    Ok(tes) => {
                        match tes.list_tasks(Some(parameters)).await {
                            Ok(task_response) => {
                                println!("{}", format_tasks_response(&task_response)); 
                            },
                            Err(e) => {
                                eprintln!("Error listing tasks: {}", e);
                            }
                        }
                    },
                    Err(e) => {
                        error!("Error creating TES instance: {:?}", e);
                        return Err(e);
                    }
                };
            }
            if let Some(("get", sub)) = sub.subcommand() {
                let mut config = Configuration::default();
                let id = sub.value_of("id").unwrap();
                let view = sub.value_of("view").unwrap();

                // let mut config = load_configuration();
                let funnel_url = ensure_funnel_running().await;
                let funnel_url = url::Url::parse(&funnel_url).expect("Invalid URL");
                config.set_base_path(funnel_url);
                match TES::new(&config).await {
                    Ok(tes) => {
                        let task = tes.get(view, id).await;
                        println!("{:?}", task);
                    }
                    Err(e) => {
                        error!("Error creating TES instance: {:?}", e);
                        return Err(e);
                    }
                };
            }
            if let Some(("status", sub)) = sub.subcommand() {
                let mut config = Configuration::default();
                let id = sub.value_of("id").unwrap().to_string();

                // let mut config = load_configuration();
                let funnel_url = ensure_funnel_running().await;
                let funnel_url = url::Url::parse(&funnel_url).expect("Invalid URL");
                config.set_base_path(funnel_url);
                let transport = Transport::new(&config);
                let task = Task::new(id.clone(), transport);
                match task.status().await {
                    Ok(status) => {
                        println!("TASKID: {}", id.clone());
                        println!("STATUS: {:?}", status);
                    }
                    Err(e) => {
                        error!("Error creating Task instance: {:?}", e);
                        return Err(e);
                    }
                };
            }
            if let Some(("cancel", sub)) = sub.subcommand() {
                let mut config = Configuration::default();
                let id = sub.value_of("id").unwrap().to_string();

                // let mut config = load_configuration();
                let funnel_url = ensure_funnel_running().await;
                let funnel_url = Url::parse(&funnel_url).expect("Invalid URL");
                config.set_base_path(funnel_url);
                let transport = Transport::new(&config);
                let task = Task::new(id, transport);
                match task.cancel().await {
                    Ok(output) => {
                        println!("The new value is: {:?}", output);
                    }
                    Err(e) => {
                        error!("Error creating Task instance: {:?}", e);
                        return Err(e);
                    }
                };
            }
        }

        _ => {
            error!("Error: Unrecognized command or option");
        }
    }
    Ok(())
}