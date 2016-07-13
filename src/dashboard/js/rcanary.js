(function () {
  'use strict';

  var serverAddress = 'ws://127.0.0.1:8099';

  function makeConnection (ws) {
    var retryHandlerID;

    ws.onopen = function () {
      console.log('Connection to ' + serverAddress + ' established');

      if (retryHandlerID !== null) {
        clearInterval(retryHandlerID);
        retryHandlerID = null;
      }
    };

    ws.onerror = function (e) {
      console.log('Connection error', e);
    };

    ws.onclose = function (e) {
      console.log('Connection closed', e);

      if (typeof retryHandlerId === 'undefined' || retryHandlerID === null) {
        retryHandlerID = setInterval(function () {
          console.log('Attempting reconnect...');
          makeConnection(new WebSocket(serverAddress));
        }, 10000);
      }
    };

    ws.onmessage = function (message) {
      console.log('Message received', message);

      var payload;

      try {
        payload = JSON.parse(message.data);
      } catch (e) {
        console.log('Invalid message', e);
        return;
      }

      document.querySelector('#root').textContent += JSON.stringify(payload);
    };

    return ws;
  }

  makeConnection(new WebSocket(serverAddress));
}());
