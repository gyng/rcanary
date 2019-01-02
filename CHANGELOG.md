# 0.5.0 (2019-01-02)

* [BREAKING] Move `health_check_server` to `health_check.enabled` and `health_check.address`
* Add `tag_metric` to targets
* Add `latency_ms` to probe results
* Add Prometheus support, configure using `metrics.enabled`, `metrics.address`
* Removed `hyper` as a direct dependency

# 0.4.0 (2018-11-08)

* [BREAKING] Moved `config.alert` to `config.alert.email`
* Add notifications to dashboard: `http://rcanary.example.com?notifications=true`

# 2017-07-24

* [BREAKING] Dashboard server specification no longer uses the URL hash. It now uses a `server` URL query parameter `http://rcanary.example.com?server=ws://localhost:8099`
* Added optional `tag` param to probe targets
* Added filter option to dashboard to filter using regex by tag `http://rcanary.example.com?filter=my-regex`
* Added "Last OK seen at" to dashboard
