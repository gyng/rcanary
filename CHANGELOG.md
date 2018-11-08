# 2018-11-08

* [BREAKING] Moved `config.alert` to `config.alert.email`

# 2017-07-24

* [BREAKING] Dashboard server specification no longer uses the URL hash. It now uses a `server` URL query parameter `http://rcanary.example.com?server=ws://localhost:8099`
* Added optional `tag` param to probe targets
* Added filter option to dashboard to filter using regex by tag `http://rcanary.example.com?filter=my-regex`
* Added "Last OK seen at" to dashboard
