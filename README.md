# Bansu

Server-side computation API for [Moorhen](https://github.com/moorhen-coot/Moorhen).

## Design

### Description

The server allows you to spawn jobs using HTTP POST requests.
Each job gets assigned an ID (being an UUID) which can later be used
to trace the job's progress and to get the computation results.

You can trace job progress by listening on a web socket connection (documented below).

The ID of your job will be valid for the following amount of time:

```
2 * T
```

where `T` is the timeout value associated with a given job type.

The (current) default timeout for `Acedrg` is 2 minutes.

### API

The server exposes the following API:

### HTTP POST `/run_acedrg"`

Creates `Acedrg` job.
Accepts the following JSON payload:

```json
{
    "smiles": "Your SMILES string",
    /// An array of additional arguments passed to acedrg
    "commandline_args: ["-z", "--something"]
}
```

Replies with the following JSON:

```json
{
    /// Null on error
    "job_id": "UUID of your job",
    /// Null on success
    "error_message": "Error message if the request failed"
}
```

Returns `201 Created` on success.

### WEBSOCKET (HTTP GET) `/ws/{job_id}`



### HTTP GET `/get_cif/{job_id}`

## Name

The name is supposed to mean "Moorhen's Nest" in Japanese.

The kanji for "Moorhen" is "[鷭](https://jisho.org/word/%E9%B7%AD-1)".[^1]

The kanji for "nest" is "巣".

Combined together gives us:

<ruby>
<rb>鷭</rb>
<rt>ban</rt>
<rb>巣</rb>
<rt>su</rt>
</ruby>


[^1]: Note: A Japanese person will likely not recognize this character. It's rarely used.

## Todo

* Isolating jobs in containers
* Input validation!!!
* Support for servalcat
* Think about API design for defining graph-like pipelines (if we ever need that)
