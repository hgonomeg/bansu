#!/usr/bin/sh
 
curl http://localhost:8080/api-docs/openapi.json | jq > openapi/bansu.json
curl http://localhost:8080/api-docs/openapi.yaml > openapi/bansu.yaml
# npx openapi-to-md openapi/bansu.json API_DOCUMENTATION.md
# npx @redocly/cli build-docs openapi/bansu.json -o docs/redoc-static.html
npm i @scalar/openapi-to-markdown
cat > script.js <<EOF
import { createMarkdownFromOpenApi } from '@scalar/openapi-to-markdown'
import { readFile, writeFile } from 'node:fs/promises';

const content = await readFile('openapi/bansu.json', { encoding: 'utf8' });

// Generate Markdown from an OpenAPI document
const markdown = await createMarkdownFromOpenApi(content);
await writeFile('API_DOCUMENTATION.md', markdown, { encoding: 'utf8' });

EOF
node script.js
rm script.js