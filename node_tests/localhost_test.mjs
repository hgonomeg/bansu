import WebSocket from 'ws';
import * as http from 'http';


const addr = process.env.BANSU_ADDRESS ? process.env.BANSU_ADDRESS : "localhost";
const port = process.env.BANSU_PORT ? process.env.BANSU_PORT : "8080";

const postData = JSON.stringify({
    'smiles': process.env.BANSU_TEST_SMILES ? process.env.BANSU_TEST_SMILES : 'c1ccccc1',
    'commandline_args': process.env.BANSU_TEST_ACEDRG_ARGS ? eval(process.env.BANSU_TEST_ACEDRG_ARGS) : []
});
  
const options = {
  hostname: addr,
  port: port,
  path: '/run_acedrg',
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
  //   'Content-Length': Buffer.byteLength(postData),
  },
};

function get_cif(job_id) {
  http.get(`http://${addr}:${port}/get_cif/${job_id}`, res => {
    let data = [];
    console.log('Status Code: ', res.statusCode);
    if(res.statusCode != 200) {
      console.error("/get_cif failed!");
      process.exit(6);
    }
  
    res.on('data', chunk => {
      data.push(chunk);
    });
  
    res.on('end', () => {
      //console.log('CIF downloaded: ');
      let cif_file_string = Buffer.concat(data).toString();
      console.log('CIF file length: ', cif_file_string.length);
      process.exit(0);
    });
  }).on('error', err => {
    console.log('Error: ', err.message);
    process.exit(7);
  });
}

function open_ws_connection(data) {
  try {
    const jsonData = JSON.parse(data);
    console.log("Got json: ", jsonData);
    if(jsonData.job_id === null) {
      console.error("Server returned null job id. Error message is: ", jsonData.error_message);
      console.log("Exiting");
      process.exit(4);
    }
    console.log("Establishing WebSocket connection.");
    // Create WebSocket connection.
    const socket = new WebSocket(`ws://${addr}:${port}/ws/${jsonData.job_id}`);


    // Connection opened
    socket.addEventListener("open", (event) => {
        console.log("Connection on WebSocket established.");
    });

    socket.addEventListener("close", (event) => {
        console.log("Connection on WebSocket closed.");
        // process.exit(0);
    });

    socket.addEventListener("error", (event) => {
        console.error("Connection on WebSocket errored-out: ", event);
        process.exit(3);
    });

    // Listen for messages
    socket.addEventListener("message", (event) => {
      try {
        const wsJson = JSON.parse(event.data);
        if(wsJson.status == "Finished") {
          const stdout_len = wsJson.job_output.stdout.length;
          const stderr_len = wsJson.job_output.stderr.length;
          console.log(`Job has finished successfully! stdout_len: ${stdout_len} stderr_len: ${stderr_len}`);
          get_cif(jsonData.job_id);
        } else if(wsJson.status == "Failed") {
          console.log(`Job failed! ${wsJson.job_output}\n\nError message:${wsJson.error_message}\nFailure reason: ${wsJson.failure_reason}`);
          process.exit(5);
        } else {
          console.log("Websocket message from server ", event.data);
        }
        } catch (e) {
          console.error(e);
        }

    });
  } catch(e) {
    console.error(e);
  }
}

const req = http.request(options, (res) => {
  console.log(`STATUS: ${res.statusCode}`);
  console.log(`HEADERS: ${JSON.stringify(res.headers)}`);
  let data = '';
  res.setEncoding('utf8');
  res.on('data', (chunk) => {
    //console.log(`BODY: ${chunk}`);
    data += chunk;
  });
  res.on('end', () => {
    //console.log('No more data in response.');
    open_ws_connection(data);
  });
});

req.on('error', (e) => {
  console.error(`Problem with HTTP request: ${e.message}`);
  process.exit(2);
});
  
// Write data to request body
req.write(postData);
req.end();



// while(true) {

// }
