import { createMarkdownFromOpenApi } from '@scalar/openapi-to-markdown'
import { readFile, writeFile } from 'node:fs/promises';

const content = await readFile('../openapi/bansu.json', { encoding: 'utf8' });

// Generate Markdown from an OpenAPI document
const markdown = await createMarkdownFromOpenApi(content);
await writeFile('../API_DOCUMENTATION.md', markdown, { encoding: 'utf8' });