import WebSocket from 'ws';
import * as https from 'https';
import * as http from 'http';
import * as crypto from 'crypto';
import * as fs from 'fs';

function get_addr_port_path_and_protocol() {
  const url = new URL(process.env.BANSU_URL ? process.env.BANSU_URL : "http://localhost:8080");
  if (url.port == null || url.port == undefined || url.port == '') {
    // If no port is specified, default to 80 for http and 443 for https
    url.port = url.protocol === 'https:' ? '443' : '80';
  }
  return {
    addr: url.hostname,
    m_path: url.pathname == '/' ? '' : url.pathname, // Remove trailing slash if present
    port: url.port,
    protocol: url.protocol
  };
}

const { addr, m_path, port, protocol } = get_addr_port_path_and_protocol();
console.log(`Using protocol: ${protocol.replace(':', '')}, address: ${addr}, path: ${m_path}, port: ${port}`);

function make_post_data() {
  if (process.env.BANSU_TEST_MMCIF) {
    // check if CIF file exists and is not empty
    if (fs.existsSync(process.env.BANSU_TEST_MMCIF) && fs.statSync(process.env.BANSU_TEST_MMCIF).size > 0) {
      // read and base64-encode mmCIF file
      const mmcif_data = fs.readFileSync(process.env.BANSU_TEST_MMCIF, { encoding: 'utf-8' });
      console.log(`Using mmCIF file: ${process.env.BANSU_TEST_MMCIF} as input, length: ${mmcif_data.length}`);
      const mmcif_base64 = Buffer.from(mmcif_data).toString('base64');
      return JSON.stringify({
        'input_mmcif_base64': mmcif_base64,
        'commandline_args': process.env.BANSU_TEST_ACEDRG_ARGS ? eval(process.env.BANSU_TEST_ACEDRG_ARGS) : []
      });

    } else {
      // We are using SMILES for testing.
      return JSON.stringify({
        'smiles': process.env.BANSU_TEST_SMILES ? process.env.BANSU_TEST_SMILES : 'c1ccccc1',
        'commandline_args': process.env.BANSU_TEST_ACEDRG_ARGS ? eval(process.env.BANSU_TEST_ACEDRG_ARGS) : []
      });

    }
  }
}

const postData = make_post_data();

const options = {
  hostname: addr,
  port: port,
  protocol: protocol,
  path: m_path + '/run_acedrg',
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    //   'Content-Length': Buffer.byteLength(postData),
  },
};

function get_cif(job_id) {

  const err_handler = err => {
    console.log('Error: ', err.message);
    process.exit(7);
  };

  const m_url = `${protocol}//${addr}:${port}${m_path}/get_cif/${job_id}`;
  const req_callback = res => {
    let data = [];
    console.log('Status Code: ', res.statusCode);
    if (res.statusCode != 200) {
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
      try {
        const hash = crypto.createHash('sha256').update(cif_file_string).digest('hex');
        console.log("CIF file SHA256 hash: ", hash);
      } catch (err) {
        console.error("Error creating hash: ", err);
        // process.exit(8);
      }
      process.exit(0);
    });
  };
  
  if (protocol === 'https:') {
    https.get(m_url, req_callback).on('error', err_handler);
  } else {
    http.get(m_url, req_callback).on('error', err_handler);
  }
}

function open_ws_connection(data) {
  try {
    const jsonData = JSON.parse(data);
    console.log("Got json: ", jsonData);
    if (jsonData.job_id == null || jsonData.job_id == undefined) {
      console.error("Server returned null job id. Error message is: ", jsonData.error_message);
      console.log("Exiting");
      process.exit(4);
    }
    console.log("Establishing WebSocket connection.");
    // Create WebSocket connection.
    let websocket_protocol = protocol === 'https:' ? 'wss' : 'ws';
    console.log(`WebSocket protocol: ${websocket_protocol}`);
    // Use the same address and port as the HTTP request
    const socket = new WebSocket(`${websocket_protocol}://${addr}:${port}${m_path}/ws/${jsonData.job_id}`);


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
        if (wsJson.status == "Finished") {
          const stdout_len = wsJson.job_output.stdout.length;
          const stderr_len = wsJson.job_output.stderr.length;
          console.log(`Job has finished successfully! stdout_len: ${stdout_len} stderr_len: ${stderr_len}`);
          console.info("Job output JSON raw: ", wsJson.job_output);
          get_cif(jsonData.job_id);
        } else if (wsJson.status == "Failed") {
          console.log(`Job failed! \nOutput: ${JSON.stringify(wsJson.job_output)}\n\nError message: ${wsJson.error_message}\nFailure reason: ${wsJson.failure_reason}`);
          process.exit(5);
        } else {
          console.log("Websocket message from server ", event.data);
        }
      } catch (e) {
        console.error(e);
      }

    });
  } catch (e) {
    console.error(e);
  }
}

const request_function = (res /*: http.IncomingMessage*/) => {
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
    if (res.statusCode > 299 || res.statusCode < 200) {
      console.error("Server returned non-2** status code. Content is:\n", data);
      process.exit(1);
    }
    open_ws_connection(data);
  });
};

let req;
if (protocol === 'https:') {
  console.log("Using HTTPS protocol for the request.");
  req = https.request(options, request_function);
} else {
  console.log("Using HTTP protocol for the request.");
  req = http.request(options, request_function);
}

req.on('error', (e) => {
  console.error(`Problem with HTTP request: ${e.name} ${e.message} caused by: ${e.cause}`);
  process.exit(2);
});

// Write data to request body
req.write(postData);
req.end();



// while(true) {

// }
