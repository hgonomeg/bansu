# Bansu

Server-side computation API for [Moorhen](https://github.com/moorhen-coot/Moorhen).

Currently it is mostly useful for running Acedrg computations on a server.

## Design

### Basic Description

The server allows you to spawn jobs using HTTP POST requests.
Each job gets assigned an ID (being an UUID) which can later be used
to trace the job's progress and to get the computation results.

After a job is spawned, you can trace job progress by listening on a WebSocket connection (documented below).

The ID of your job will be valid for the following amount of time:

```
2 * T
```

where `T` is the timeout value associated with a given job type.

The (current) default timeout for `Acedrg` is 2 minutes.

All data is deleted after job's ID becomes invalid i.e. expires.

### Security

All data (temporary directories with all artifacts) is automatically deleted after job's ID becomes invalid.
Whomever has the ID, can access your data. Treat the ID as both an identifier and a security token.

Keep in mind that not sharing your ID token does not prevent the server administrator from being able to see your data.
For maximum data security, you may want to run your own instance of Bansu.

For security reasons, the server also supports running jobs
in Docker containers which is the recommended thing to do.

Running jobs in a Docker container helps to isolate jobs from each other and the rest of the system.
This greatly reduces the potential impact of a successful exploitation of any vulnerabilities present in Acedrg and Servalcat.

## Configuration

The following environment variables control the behavior of the server:

* `TMPDIR` - can be used to specify location where jobs store temporary files
* `RUST_LOG` - sets the log level (like with most Rust programs)
* `BANSU_PORT` - sets the port to listen on (`8080` by default)
* `BANSU_ADDRESS` - sets the address to listen on (`127.0.0.1` by default)
* `BANSU_BASE_URL` - sets the base HTTP path, e.g. (`/bansu`)
* `BANSU_DOCKER` - enables Docker support and sets the name of the Docker image used for running jobs. If this variable is set, the server will refuse to run if the Docker configuration is invalid
* `BANSU_DISALLOW_DOCKERLESS` - can be set to cause Bansu to refuse to run without Docker support
* `BANSU_PERIODIC_WS_UPDATE_INTERVAL` - specifies the interval at which web socket connections send periodic status updates (in seconds; `16` by default)
* `BANSU_ACEDRG_TIMEOUT` - specifies timeout for Acedrg (in seconds) (`120` by default)
* `BANSU_MAX_CONCURRENT_JOBS` - specifies the maximum number of jobs running in parallel (`20` by default). Use `0` to disable limit.
* `BANSU_MAX_JOB_QUEUE_LENGTH` - specifies the maximum number of jobs waiting in queue to be processed. (`20` by default). Use `0` to disable job queueing.
* `BANSU_RATELIMIT_BURST_SIZE` - specifis [burst size for rate limiter](https://docs.rs/actix-governor/0.8.0/actix_governor/struct.GovernorConfigBuilder.html#method.burst_size) (per IP address; `45` by default)
* `BANSU_RATELIMIT_SECONDS_PER_REQUEST` - specifies the [interval (in seconds) after which the rate limiter replenishes quota element](https://docs.rs/actix-governor/0.8.0/actix_governor/struct.GovernorConfigBuilder.html#method.seconds_per_request) (`10` by default). Divide 60 by this number to arrive at requests per minute per IP address.
* `BANSU_DISABLE_RATELIMIT` - can be set to disable rate-limiting of requests. (For testing only)
* `BANSU_DISABLE_APIDOC` - Disable json/yaml OpenAPI documentation at `/api-docs/openapi.json`
* `BANSU_USAGE_STATS_DB` - Sets the DB connection ([as defined here](https://www.sea-ql.org/SeaORM/docs/install-and-config/connection/)) used for storing usage data (Not used by default)

## API

Below is my hand-written documentation.
For an experimental auto-generated documentation, [click here](API_DOCUMENTATION.md).
The server exposes the following API:

### HTTP GET `/vibe_check`

Health check endpoint.

Returns the following JSON:

```json5
{
    /// Bansu version
    "bansu_version": "0.4.0",
    /// Current length of the queue or null if queue disabled
    "queue_length": 12,
    /// Max length of the queue or null if queue disabled
    "max_queue_length": 30,
    /// Number of jobs currently being processed (or still available for downloading job results)
    "active_jobs": 13,
    /// Max number of jobs to be run in parallel
    "max_concurrent_jobs": 10,
    /// Uptime in seconds
    "uptime": 986986
}
```

Returns:

* `200 OK` - All is ok.


### HTTP POST `/run_acedrg`

Creates `Acedrg` job.
Accepts the following JSON payload:

```json5
{
    /// Input SMILES string (only one input should be present at a time)
    "smiles": "Your SMILES string",
    /// Input mmCIF file, base64-encoded (only one input should be present at a time)
    "input_mmcif_base64": "",
    /// CCD code for fetching the input structure from the PDBe (only one input should be present at a time)
    "ccd_code": "ALA",
    /// An array of additional arguments passed to acedrg
    /// Note: not all Acedrg arguments are currently available
    "commandline_args": ["-z", "--something"]
}
```

Replies with the following JSON:

```json5
{
    /// Null on error
    "job_id": "UUID of your job",
    /// Null on success
    "error_message": "Error message if the request failed",
    /// Position counted from 0
    /// Null if the job is being processed without a queue
    "queue_position": 12
}
```

Returns:

* `201 Created` on success (job spawned)
* `202 Accepted` if job has been queued
* `400 Bad Request` on input validation error
* `503 Service Unavailable` if the server is currently at capacity and is unable to handle your request
* `500 Internal Server Error` on all other kinds of errors

### WEBSOCKET (HTTP GET) `/ws/{job_id}`

Opens a websocket connection which allows you to track the job's progress.
Progress reports have the following JSON format:

```json5
{
    "status": "Pending | Finished | Failed | Queued",
    /// Only not-null if the job is queued
    "queue_position": "number | null",
    /// Will be null if the job is still pending, if it timed-out
    /// or the child process failed due to an I/O error
    "job_output": {
        "stdout": "A string",
        "stderr": "A string"
    },
    /// Can be not-null only if the job failed.
    /// Currently, this only has value for `SetupError` failure reason
    "error_message": "Some error message",
    /// Only not-null if the job failed
    "failure_reason": "TimedOut | JobProcessError | SetupError"
}

```

Progress messages are sent in the following scenarios:

* When the connection gets established
* When the job status / `job_output` get updated
* Periodically, with interval specified in server configuration (useful for estimating how long it may take for a queued job to be processed)

Connection gets automatically closed if the job fails or completes.

For queued jobs which could not have been started, listening on a WebSocket also lets you know why it failed (including input validation failure and all oher errors).

The connection ignores all messages sent to it (responds only to Ping messages).

Returns `404 Not Found` if the given `job_id` is not valid.


### HTTP GET `/get_cif/{job_id}`

For Acedrg jobs, streams the contents of the CIF file generated by Acedrg.

Returns:

* `200 OK` on success
* `404 Not Found` if the given `job_id` is not valid.
* `500 Internal Server Error` if the output file could not be read
* `400 Bad Request` if the given `job_id` does not correspond to an `Acedrg` job or if the job is still pending (or queued)

## Setup

### Build and run

In order to build Bansu, all you need is a Rust compiler.
Just use:

`$ cargo build -r`

No special compile-time dependencies are needed.

There is one optional runtime dependency: Docker.

The server manages docker containers (as described above in the security section).
It needs to have adequate permissions in order to do that.
The server uses the `bollard` crate to setup Docker connection using platform-dependent defaults (Unix pipe, Windows socket, fallback: HTTP).
Refer to [bollard documentation for more details](https://docs.rs/bollard/0.19.2/bollard/struct.Docker.html#method.connect_with_defaults).

If you do not want to make use of Docker support, make sure that `acedrg` and `servalcat` are available in the system path.

### Docker container setup

A `Dockerfile` is included to build a suitable Docker container image.
The image is based on Fedora but can be used on any distribution.

In order to build it:

1. Go to `docker/`
2. Run `docker build --pull --network host -t <name_of_your_image> -f FedoraDockerfile .`
3. Wait for the image to be built.

#### Docker UID & permissions

**Note:** By default, job artifacts will be owned by the user with **UID=1000**
If this is not what you want, edit the `Dockerfile` and change the UID of the `bansu_container` user!
(Defined by: `RUN useradd bansu_container -u 1000 -m -s /bin/bash`)

### Testing / Usage example

In order to test Bansu, you can make use of the provided Node.JS script.

0. Launch the Bansu server
1. Go to `node_tests/`
2. Run `npm install` to fetch Node dependencies
3. Run `node test_acedrg_job.mjs` - it will spawn an Acedrg job, then wait until it finishes and try to get CIF.

Environment variables for the test:

* `BANSU_URL` - the URL to be used for Bansu connection (default: `http://localhost:8080`)
* `BANSU_TEST_SMILES` - SMILES string used for testing (default: `c1ccccc1`)
* `BANSU_TEST_MMCIF` - Specifies mmCIF file name to be used as Acedrg input (instead of working with SMILES)
* `BANSU_TEST_CCD_CODE` - Specifies CCD code to be fetched and used as Acedrg input (instead of working with SMILES or an mmCIF file)
* `BANSU_TEST_ACEDRG_ARGS` - Args for Acedrg (default: `[]`) This is a JSON array of commandline arguments. 

Feel free to hack with the script to adjust it to your needs.

You can also use the script as a reference for implementing communication with Bansu in your project.

## Name

The name is supposed to mean "Moorhen's Nest" in Japanese.

The kanji for "Moorhen" is "[鷭](https://jisho.org/word/%E9%B7%AD-1)".[^1]

The kanji for "nest" is "巣".

Combined together gives us:

<!-- <span style="font-size: 200%"> -->
<ruby>
<rb>鷭</rb>
<rt>ban</rt>
<rb>巣</rb>
<rt>su</rt>
</ruby>
<!-- </span> -->


[^1]: Note: A Japanese person will likely not recognize this character. It's rarely used.

## Todo

* Update job output in realtime
* Some way of achieving interactivity (admin console?)
* Graceful shutdown (finishing current jobs + what's already in the queue)
* Maintaining a database of docker containers + temporary directories (for automatic cleanup after dirty shutdown)
* Support for servalcat
* Think about API design for defining graph-like pipelines (if we ever need that)
* Add api versioning (`/v1`, `/v2`) - when we need it