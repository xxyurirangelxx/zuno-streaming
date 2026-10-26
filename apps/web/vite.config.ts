import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// Porta 8080 pode estar ocupada por outro software no Windows; permita override
// sem alterar o contrato padrão (ZUNO_BIND_ADDR=127.0.0.1:8080).
const apiTarget = process.env.ZUNO_DEV_API_TARGET ?? 'http://127.0.0.1:8080';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
    proxy: {
      '/health': {
        target: apiTarget,
        changeOrigin: false,
      },
      // Mesma origem no dev: o cookie de sessão `HttpOnly` do backend funciona
      // sem configuração de CORS nem `credentials` cross-origin.
      '/api': {
        target: apiTarget,
        changeOrigin: false,
      },
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    css: true,
    restoreMocks: true,
  },
});
