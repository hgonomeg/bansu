CREATE TABLE Requests (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT
                               NOT NULL,
    api_route          TEXT    NOT NULL,
    successful         INTEGER NOT NULL,
    ip_address         INTEGER NOT NULL,
    time_to_process    INTEGER NOT NULL,
    job_queue_len      INTEGER NOT NULL,
    num_of_job_running INTEGER NOT NULL
);

CREATE TABLE Jobs (
    id              INTEGER  PRIMARY KEY AUTOINCREMENT
                             UNIQUE,
    start_time      DATETIME NOT NULL,
    processing_time INTEGER,
    ip_address      INTEGER  NOT NULL,
    successful      INTEGER
);


