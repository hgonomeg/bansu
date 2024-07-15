import WebSocket from 'ws';
import * as http from 'http';


const postData = JSON.stringify({
    'smiles': 'c1ccccc1',
    'commandline_args': []
  });
  
  const options = {
    hostname: 'localhost',
    port: 8080,
    path: '/run_acedrg',
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    //   'Content-Length': Buffer.byteLength(postData),
    },
  };
  
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
      try {
        const jsonData = JSON.parse(data);
        console.log("Got json: ", jsonData);
        console.log("Establishing WebSocket connection.");
        // Create WebSocket connection.
        const socket = new WebSocket(`ws://localhost:8080/ws/${jsonData.job_id}`);


        // Connection opened
        socket.addEventListener("open", (event) => {
            console.log("Connection on WebSocket established.");
        });

        socket.addEventListener("close", (event) => {
            console.log("Connection on WebSocket closed.");
            process.exit(0);
        });

        socket.addEventListener("error", (event) => {
            console.error("Connection on WebSocket errored-out: ", event);
            process.exit(3);
        });

        // Listen for messages
        socket.addEventListener("message", (event) => {
            console.log("Websocket message from server ", event.data);
        });
      } catch(e) {
        console.error(e);
      }
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