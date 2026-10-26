import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
export type Food = {
    id: string;
    name: string;
    createdAt: string;
    unlimited: boolean;
    price: number;
    quantity: number;
    revision: number;
    updatedAt: string;
};
export type Preferences = {
    defaultFoodId: string;
    favoriteFoodId: string | null;
};
export type FoodReceipt = {
    actionId: string;
    kind: string;
    foodId: string;
    quantity: number;
    personaId: string | null;
    sessionId: string | null;
    favoriteBonus: boolean;
    intimacyDelta: number;
    coinDelta: number;
    replayed: boolean;
};
export type Skill = {
    id: string;
    title: string;
    summary: string;
    body: string;
    revision: number;
    createdAt: string;
    updatedAt: string;
    personas: {
        id: string;
        name: string;
    }[];
};
export const companionApi = {
    parseSkillMarkdown: (filename:string,body:string) => invoke<{title:string;summary:string;body:string}>("parse_skill_markdown",{filename,body}),
    cancelSkillMarkdownImport: () => invoke<void>("cancel_skill_markdown_import"),
    listenSkills: (receive: () => void) => listen("skills-changed", receive),
    skillPreviews: (context: {personaId:string;sessionId:string;contextRevision:number}) => invoke<SkillPreview[]>("get_skill_previews", { context }),
    resolveSkillPreview: (context: {personaId:string;sessionId:string;contextRevision:number}, id:string, accept:boolean) => invoke<void>("resolve_skill_preview", { context, id, accept }),
    listenFoods: (receive: () => void) => listen("foods-changed", receive),
    foods: () => invoke<Food[]>("list_foods"), preferences: (personaId: string) => invoke<Preferences>("get_food_preferences", { personaId }),
    savePreferences: (personaId: string, p: Preferences, field: "default" | "favorite") => invoke<void>("save_food_preferences", { personaId, ...p, field }),
    buy: (actionId: string, foodId: string, quantity: number, quote: number) => invoke<FoodReceipt>("buy_food", { actionId, foodId, quantity, quote }),
    receipt: (actionId: string) => invoke<FoodReceipt | null>("get_food_action", { actionId }),
    feed: (actionId: string, foodId: string, context: {
        personaId: string;
        sessionId: string;
        contextRevision: number;
    }) => invoke<FoodReceipt>("feed_food", { actionId, foodId, ...context }),
    skills: () => invoke<Skill[]>("list_skills"), saveSkill: (input: {
        id: string | null;
        revision: number | null;
        title: string;
        summary: string;
        body: string;
    }) => invoke<void>("save_skill", { input }),
    deleteSkill: (id: string, revision: number) => invoke<void>("delete_skill", { id, revision }), bind: (skillId: string, personaId: string, enabled: boolean) => invoke<void>("bind_skill", { skillId, personaId, enabled }),
};
export type SkillPreview = { id:string; before:Skill; proposed:{title:string;summary:string;body:string} };
const images = import.meta.glob<string>("../resources/foods/*.png", { eager: true, query: "?url", import: "default" });
export function foodImage(id: string) { return images[`../resources/foods/${id}.png`]; }
export function openSkill(id: string) { window.dispatchEvent(new CustomEvent("ym-open-skill", { detail: id })); }
