# Request logging

## Basic principles

* Job data (such as smiles strings, job results etc.) should not be logged
* Should gather all data allowing to build up a good imagination of how users interact with Bansu

## Things to log

### Per request

* Time
* Request kind (/run_acedrg, /get_cif)
* Request status (success / fail; excluding requests blocked by rate limiter)
  * Optional error message
* IP address
* Time to process HTTP request (not the job itself; in microseconds?)
* Job queue length at the time of making the request
* Number of jobs currently running

### Per job

* Time job started
* Time taken to process the job (in seconds?)
* IP address
* Status (success / failure)
  * Optional error message
* Maybe add job kind ?


