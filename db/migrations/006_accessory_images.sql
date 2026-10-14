-- v5 -> v6：仅增加可选透明图片引用；旧饰品、价格、档位、钱包及索引不变。
ALTER TABLE accessories ADD COLUMN image_file TEXT; -- 透明饰品PNG的受管相对路径；NULL使用display_text内的图标字符/Emoji/颜文字/符号组合
