(function () {
  'use strict';

  var serverAddress = 'ws://127.0.0.1:8099';
  var targets = null;
  var retryHandlerID;

  function makeConnection (ws) {
    ws.onopen = function () {
      console.log('Connection to ' + serverAddress + ' established');

      targets = null;
      document.querySelector('#root').innerHTML = '';

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
        console.log('Starting reconnect process...');
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

      // First message is the target list
      if (targets === null) {
        targets = payload;
        var template = document.querySelector('#probe-target');
        var root = document.querySelector('#root');

        targets.http.forEach(function (t) {
          template.content.querySelector('.probe-name').textContent = t.name;
          template.content.querySelector('.probe-target').dataset.host = t.host;
          var clone = document.importNode(template.content, true);
          root.appendChild(clone);
        });
      } else {
        var selector = '.probe-target[data-host="' + payload.target.host + '"]';
        var targetEl = document.querySelector(selector);
        var time = new Date(Date.parse(payload.time)).toLocaleString(undefined, { timeZoneName: 'short' });
        targetEl.dataset.status = payload.info;
        targetEl.querySelector('.probe-status').textContent = payload.status_code;
        targetEl.querySelector('.probe-time').textContent = time;
      }
    };

    return ws;
  }

  makeConnection(new WebSocket(serverAddress));
}());
