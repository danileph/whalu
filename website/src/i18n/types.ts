import type { en } from "./messages/en.ts";

export type MessageKey = keyof typeof en;
export type Messages = Record<MessageKey, string>;
type Placeholders<S extends string> =
  S extends `${string}{${infer P}}${infer Rest}`
    ? P | Placeholders<Rest>
    : never;
export type MessageParameters = Record<string, string | number>;
type Arguments<K extends MessageKey> = [Placeholders<(typeof en)[K]>] extends [
  never,
]
  ? [parameters?: never]
  : [parameters: Record<Placeholders<(typeof en)[K]>, string | number>];
export type Translator = <K extends MessageKey>(
  key: K,
  ...args: Arguments<K>
) => string;
