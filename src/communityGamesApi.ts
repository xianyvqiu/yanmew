import {invoke} from "@tauri-apps/api/core";
export type CommunityGame={id:string;name:string;description:string;author:string;version:string;apiVersion:string;repository:string;readmeUrl:string;readmeSha256:string;downloadUrl:string;sha256:string;sizeBytes:number;publishedAt:string};
export type CommunityReadme={markdown:string;baseUrl:string};
export const communityGamesApi={
  list:()=>invoke<CommunityGame[]>("list_community_games"),
  readme:(id:string,version:string)=>invoke<CommunityReadme>("get_community_game_readme",{id,version}),
  download:(id:string,version:string)=>invoke<string>("get_community_game_download",{id,version}),
};
export const communityRepository="https://github.com/xianyvqiu/yanmew-community-games";
