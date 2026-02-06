#![allow(unused_imports)]
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("Requests")
                    .if_not_exists()
                    .col(
                        ColumnDef::new("id")
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new("time_sent").date_time().not_null())
                    .col(ColumnDef::new("api_route").text().not_null())
                    .col(ColumnDef::new("successful").integer().not_null())
                    .col(ColumnDef::new("ip_address").binary().not_null())
                    .col(ColumnDef::new("time_to_process").integer().not_null())
                    .col(ColumnDef::new("job_queue_len").integer().not_null())
                    .col(ColumnDef::new("num_of_jobs_running").integer().not_null())
                    .col(ColumnDef::new("error_message").text())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table("Jobs")
                    .if_not_exists()
                    .col(
                        ColumnDef::new("id")
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(), // .unique_key(),
                    )
                    .col(ColumnDef::new("job_id").text())
                    .col(ColumnDef::new("start_time").date_time().not_null())
                    .col(ColumnDef::new("processing_time").integer())
                    .col(ColumnDef::new("ip_address").binary().not_null())
                    .col(ColumnDef::new("successful").integer())
                    .col(ColumnDef::new("error_message").text())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("Requests").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table("Jobs").to_owned())
            .await
    }
}
