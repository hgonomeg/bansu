import WebSocket from 'ws';

// Create WebSocket connection.
const socket = new WebSocket("ws://localhost:8080/bansu_ws");

// Connection opened
socket.addEventListener("open", (event) => {
    console.log("Connection established.");
    
});

socket.addEventListener("close", (event) => {
    console.log("Connection closed.");
});

socket.addEventListener("error", (event) => {
    console.error("Connection errored-out: {}", event);
});

// Listen for messages
socket.addEventListener("message", (event) => {
  console.log("Message from server ", event.data);
});

// while(true) {

// }