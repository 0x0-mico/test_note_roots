/// <reference types="vite/client" />

declare module '*.masm?raw' {
  const src: string
  export default src
}
