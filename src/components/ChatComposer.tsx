import {useEffect,useRef,useState,useId,type FormEvent,type KeyboardEvent} from "react";
import type {ChatImageInput} from "../types";
import "./ChatComposer.css";

type ChatComposerProps = {
  compact?: boolean;
  images?:ChatImageInput[];
  onImagesChange?:(images:ChatImageInput[])=>void;
  attachmentOwnerKey?:string;
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  onCancel: () => void;
  disabled: boolean;
  busy: boolean;
  cancelling: boolean;
};

export default function ChatComposer({
  value,
  compact=false,
  images=[],onImagesChange,attachmentOwnerKey="",
  onChange,
  onSend,
  onCancel,
  disabled,
  busy,
  cancelling,
}: ChatComposerProps) {
  const inputId=useId();
  const inputElement=useRef<HTMLTextAreaElement>(null);
  useEffect(()=>{if(compact&&!disabled)inputElement.current?.focus();},[compact,disabled]);
  const [reading,setReading]=useState(false);const [imageError,setImageError]=useState("");
  const picker=useRef<HTMLInputElement>(null);const alive=useRef(true);const owner=useRef(attachmentOwnerKey);owner.current=attachmentOwnerKey;
  const currentImages=useRef(images);currentImages.current=images;
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  useEffect(()=>{setImageError("");setReading(false);},[attachmentOwnerKey]);
  const addImages=async(files:FileList|null)=>{
    if(!files||!onImagesChange||disabled||busy||reading)return;
    const key=owner.current;setReading(true);setImageError("");
    try{const added=await Promise.all(Array.from(files).map(file=>new Promise<ChatImageInput>((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve({filename:file.name,dataUrl:String(reader.result)});reader.onerror=()=>reject(new Error(`无法读取 ${file.name}`));reader.readAsDataURL(file);})));if(alive.current&&owner.current===key)onImagesChange([...currentImages.current,...added]);}
    catch(cause){if(alive.current&&owner.current===key)setImageError(cause instanceof Error?cause.message:"图片读取失败");}
    finally{if(alive.current&&owner.current===key)setReading(false);}
  };
  const canSend = !disabled && !busy && !reading && Boolean(value.trim()||images.length);

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (canSend) onSend();
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey) return;
    if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return;

    event.preventDefault();
    if (canSend) onSend();
  };

  return (
    <form className={`chat-composer${compact ? " chat-composer--compact" : ""}`} onSubmit={submit}>
      <label className="chat-composer__label" htmlFor={inputId}>
        消息
      </label>
      <textarea
        ref={inputElement}
        id={inputId}
        className="chat-composer__input"
        aria-label="聊天消息"
        placeholder="想和我聊些什么？"
        value={value}
        onChange={event => onChange(event.currentTarget.value)}
        onKeyDown={handleKeyDown}
        disabled={disabled}
        rows={compact ? 2 : 3}
        autoFocus={compact}
      />
      {images.length>0&&<div className="ym-image-drafts" aria-label="待发送图片">{images.map((image,index)=><figure key={`${image.filename}:${index}`}><img src={image.dataUrl} alt={image.filename}/><figcaption>{image.filename}</figcaption><button className="ym-button" type="button" disabled={busy||reading} onClick={()=>onImagesChange?.(images.filter((_,i)=>i!==index))}>移除图片 {index+1}</button></figure>)}</div>}
      {imageError&&<p className="ym-page-error" role="alert">{imageError}</p>}
      <div className="chat-composer__actions">
        {onImagesChange&&<><input ref={picker} type="file" hidden multiple accept="image/png,image/jpeg,image/webp" onChange={event=>{void addImages(event.target.files);event.target.value="";}}/><button className="ym-button" type="button" disabled={disabled||busy||reading} onClick={()=>picker.current?.click()}>{reading?"读取图片中…":"添加图片"}</button></>}
        {busy ? (
          <button
            className="ym-button chat-composer__submit"
            type="button"
            onClick={onCancel}
            disabled={cancelling}
          >
            {cancelling ? "正在暂停…" : "暂停回复"}
          </button>
        ) : (
          <button
            className="ym-button chat-composer__submit"
            type="submit"
            disabled={!canSend}
          >
            发送
          </button>
        )}
      </div>
    </form>
  );
}
