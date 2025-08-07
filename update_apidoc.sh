#!/usr/bin/sh
function fail() {
    echo Bansu must be running, listening on port 8080 and have openapi support enabled for this script to work.
    exit 2
}
curl http://localhost:8080/api-docs/openapi.json | jq > openapi/bansu.json || fail
curl http://localhost:8080/api-docs/openapi.yaml > openapi/bansu.yaml || fail
# npx openapi-to-md openapi/bansu.json API_DOCUMENTATION.md
# npx @redocly/cli build-docs openapi/bansu.json -o docs/redoc-static.html
pushd autogenerate_documentation
echo Fetching node dependencies... \(this may take a while\)
npm i @scalar/openapi-to-markdown
echo Autogenerating documentation...
node run-openapi-to-markdown.js
popd