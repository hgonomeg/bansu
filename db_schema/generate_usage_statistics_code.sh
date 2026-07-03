#!/usr/bin/sh
if not [ -f bansu_usage_statistics.sqlite3 ]; then
    echo "Creating usage statistics database"
    sqlite3 bansu_usage_statistics.sqlite3 < db_schema/init_statistics_db.sql
else
    echo "Usage statistics database already exists. Not generating."
fi
sea-orm-cli generate entity --database-url sqlite://bansu_usage_statistics.sqlite3 -v --date-time-crate chrono -o ./src/usage_statistics_entity