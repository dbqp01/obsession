import { McpServer } from '@modelcontextprotocol/server';
import { serveStdio } from '@modelcontextprotocol/server/stdio';
import { WorkflowError } from './errors.mjs';
import { createTemuRunner, temuDetailInput, temuSearchInput } from './runner.mjs';

const definitions = {
  temu_search_products: {
    title: 'Buscar productos en Temu',
    description: 'Busca productos en Temu usando la sesión personal iniciada en este equipo. Devuelve nombre, precio mostrado en la cuenta, imagen y enlace. Solo lectura.',
    inputSchema: temuSearchInput,
  },
  temu_product_details: {
    title: 'Ver producto de Temu',
    description: 'Lee nombre, precio mostrado en la cuenta, descripción e imagen de una URL de producto de Temu. Solo lectura.',
    inputSchema: temuDetailInput,
  },
};

export function createServer(runner = createTemuRunner()) {
  const server = new McpServer({ name: 'anyweb-temu', version: '1.0.0' });
  server.registerTool('temu_search_products', {
    title: definitions.temu_search_products.title,
    description: definitions.temu_search_products.description,
    inputSchema: definitions.temu_search_products.inputSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: true },
  }, async input => call(() => runner.search(input)));
  server.registerTool('temu_product_details', {
    title: definitions.temu_product_details.title,
    description: definitions.temu_product_details.description,
    inputSchema: definitions.temu_product_details.inputSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: true },
  }, async input => call(() => runner.detail(input)));
  return server;
}

function formatResult(data) {
  if (Array.isArray(data)) {
    if (!data.length) return 'No encontré productos que coincidan con la búsqueda.';
    return data.map((product, index) => [
      `${index + 1}. ${product.title}`,
      `   Precio: ${product.price ?? 'No mostrado'}`,
      `   Imagen: ${product.image ?? 'No disponible'}`,
      `   Enlace: ${product.url}`,
    ].join('\n')).join('\n\n');
  }
  return [
    `Producto: ${data.title ?? 'Sin nombre'}`,
    `Precio: ${data.price ?? 'No mostrado'}`,
    `Descripción: ${data.description ?? 'No disponible'}`,
    `Imagen: ${data.image ?? 'No disponible'}`,
    `Enlace: ${data.url ?? 'No disponible'}`,
  ].join('\n');
}

async function call(task) {
  try {
    const data = await task();
    return { content: [{ type: 'text', text: formatResult(data) }] };
  } catch (error) {
    const failure = error instanceof WorkflowError ? { error: error.code } : { error: 'INTERNAL_ERROR' };
    return { isError: true, content: [{ type: 'text', text: JSON.stringify(failure) }] };
  }
}

export async function serve() {
  const runner = createTemuRunner();
  for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, () => {
    void runner.close().finally(() => process.exit(0));
  });
  return serveStdio(() => createServer(runner));
}
