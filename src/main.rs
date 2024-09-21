use std::fmt::{Debug, Display};

use newsletter::issue_delivery_worker::run_worker_until_stopped;
use newsletter::startup::Application;
use newsletter::configuration::get_configuration;
use newsletter::telemetry::{get_subscriber, init_subscriber};
use tokio::task::JoinError;

#[actix_web::main]
async fn main() -> anyhow::Result<()>{
    let subscriber = get_subscriber("Newsletter".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let config = get_configuration().expect("Failed to get configuration");
    let application = Application::build(config.clone())
            .await?;

    let application_task = tokio::spawn(application.run_until_stopped());
    let worker_task = tokio::spawn(run_worker_until_stopped(config));

    tokio::select! {
        o = application_task => report_exit("API", o),
        o = worker_task => report_exit("Background Worker", o),
    }

    Ok(())
}

fn report_exit(
    task_name: &str,
    outcome: Result<Result<(), impl Debug + Display>, JoinError>
){
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        },

        Ok(Err(e)) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{} failed",
                task_name
            )
        },

        Err(e) => {
            dbg!(&e);

            tracing::error!(
            error.cause_chain = ?e,
            error.message = %e,
            "{}' task failed to complete",
            task_name
            )
        }
    }
}
