(function () {
  'use strict';

  if (!window.WebSocket) {
    document.body = 'This dashboard requires WebSocket support.';
    return;
  }

  // https://stackoverflow.com/questions/901115
  function getParameter (name, url) {
      if (!url) url = window.location.href;
      var param = name.replace(/[\[\]]/g, "\\$&");
      var regex = new RegExp("[?&]" + param + "(=([^&#]*)|&|#|$)");
      var results = regex.exec(url);
      if (!results) return null;
      if (!results[2]) return '';

      return decodeURIComponent(results[2].replace(/\+/g, " "));
  }

  function formatDatetime (datetime) {
    var locale = navigator.language ||
      navigator.browserLanguage ||
      navigator.systemLanguage ||
      navigator.userLanguage;

    return new Date(Date.parse(datetime))
      .toLocaleString(locale, { timeZoneName: 'short' });
  }

  function notify (payload, originalDataset) {
    var statusIcons = {
      Unknown: '🚨',
      Fire: '🔥',
      Okay: '✅'
    };
    var title = payload.target.name;

    var body = [
      statusIcons[payload.status] + ' ' + payload.status + ': ' + payload.status_code,
      'was ' + originalDataset.status
    ].join('\n');

    new Notification(title, {
      body: body,
      renotify: true,
    });
  }

  var customServerAddress = getParameter('server');
  var customFilter = getParameter('filter');
  var notifications = getParameter('notifications');

  var hostname = window.location.hostname;
  var protocol = window.location.protocol == 'https' ? 'wss' : 'ws';
  var defaultPort = '8099';
  var defaultServerAddress = protocol + '://' + hostname + ':' + defaultPort;

  var serverAddress = customServerAddress || defaultServerAddress;
  var filter = customFilter && new RegExp(customFilter) || /.*/;

  if (notifications === "true") {
    console.log(window.Notification)
    if (window.Notification) {
      Notification.requestPermission(function cb (result) {
        console.log("notifications request:", result);
      });
    } else {
      console.log("This browser does not support system notifications");
    }
  }

  console.log('rcanary server address: ' + serverAddress);
  console.log(customServerAddress ? 'set from URL hash' : 'set to default address as hash is empty');
  console.log('using tag filter: ' + filter);

  var targets = null;
  var retryHandlerID = null;
  var staleTimers = {};

  function makeConnection (ws) {
    ws.onopen = function () {
      console.log('Connection to ' + serverAddress + ' established');

      targets = null;
      document.querySelector('#root').innerHTML = '';
      clearInterval(retryHandlerID);
      retryHandlerID = null;
    };

    ws.onerror = function (e) {
      console.log('Connection error', e);
    };

    ws.onclose = function (e) {
      console.log('Connection closed', e);

      if (retryHandlerID === null) {
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

      if (targets === null) {
        // First message is the target list
        targets = payload;
        var template = document.querySelector('#probe-target');
        var root = document.querySelector('#root');

        targets.http
          .filter(function (t) {
            return filter.test(t.tag);
          })
          .forEach(function (t) {
            template.content.querySelector('.probe-name').textContent = t.name;
            template.content.querySelector('.probe-target').dataset.host = t.host;
            var clone = document.importNode(template.content, true);
            root.appendChild(clone);
          });
      } else {
        // Update to targets
        if (!filter.test(payload.target.tag)) {
          return;
        }

        var selector = '.probe-target[data-host="' + payload.target.host + '"]';
        var targetEl = document.querySelector(selector);
        var time = formatDatetime(payload.time);
        var timeout_s = 30000; // Rust timeout

        if (targetEl.dataset.updated != null && targetEl.dataset.status !== payload.status) {
          notify(payload, targetEl.dataset);
        }

        targetEl.dataset.status = payload.status;
        targetEl.dataset.updated = payload.time;

        targetEl.dataset.stale = false;
        clearTimeout(staleTimers[payload.target.host]);
        staleTimers[payload.target.host] = setTimeout(function () {
          targetEl.dataset.stale = true;
        }, payload.target.interval_s * 1000 * 2 + timeout_s);

        targetEl.querySelector('.probe-status').textContent = payload.status_code;
        targetEl.querySelector('.probe-time').textContent = time;
        targetEl.querySelector('.probe-link').href = payload.target.host;
        if (payload.status === 'Okay') {
          targetEl.querySelector('.probe-last-okay').textContent = 'Last OK: ' + formatDatetime(payload.time);
        }
      }
    };

    return ws;
  }

  makeConnection(new WebSocket(serverAddress));
}());
