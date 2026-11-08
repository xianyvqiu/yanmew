import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
export type StickerTag={id:string;name:string;revision:number;createdAt:string;updatedAt:string};
export type StickerLibrary={id:string;title:string;description:string;revision:number;createdAt:string;updatedAt:string;count:number;personaIds:string[]};
export type ChatSticker={id:string;libraryId:string;title:string;description:string;filename:string;mediaType:string;revision:number;createdAt:string;updatedAt:string;tags:Pick<StickerTag,"id"|"name">[]};
export type StickerQuery={entity:"libraries"|"stickers"|"tags";query?:string;page?:number;libraryId?:string|null;personaId?:string|null;tagIds?:string[]};
export type StickerPage<T>={items:T[];page:number;pages:number;total:number};
export function openStickerLibrary(id:string){window.dispatchEvent(new CustomEvent("ym-open-sticker-library",{detail:id}));}
export const stickerApi={
  query:<T>(input:StickerQuery)=>invoke<StickerPage<T>>("query_chat_stickers",{input}),
  mutate:(input:Record<string,unknown>)=>invoke<{saved:boolean;id?:string}>("mutate_chat_stickers",{input}),
  import:(libraryId:string,input:{filename:string;dataUrl:string})=>invoke<void>("import_chat_sticker",{libraryId,input}),
  image:(id:string)=>invoke<string>("get_chat_sticker_image",{id}),
  listen:(fn:()=>void)=>listen("chat-stickers-changed",fn),
};
