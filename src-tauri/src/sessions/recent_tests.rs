use super::*;
#[test]
fn latest_page_and_older_pages_are_bounded_ordered_and_without_gaps(){
 let runtime=crate::runtime::test_runtime("http://127.0.0.1:9/v1");
 runtime.read(|s,_|{
  for index in 1..=250{s.connection.execute("INSERT INTO messages(session_id,role,content,created_at) VALUES(1,'assistant',?1,'a')",[index.to_string()])?;}
  let first=recent_messages(&s.connection,1,1,None,100)?;assert_eq!(first.items.len(),100);assert_eq!(first.items[0].content,"151");assert_eq!(first.items[99].content,"250");
  let cursor=first.next_after_id.unwrap().parse().unwrap();let second=recent_messages(&s.connection,1,1,Some(cursor),100)?;assert_eq!(second.items[0].content,"51");assert_eq!(second.items[99].content,"150");
  let cursor=second.next_after_id.unwrap().parse().unwrap();let last=recent_messages(&s.connection,1,1,Some(cursor),100)?;assert_eq!(last.items.len(),50);assert_eq!(last.items[0].content,"1");assert!(last.next_after_id.is_none());
  let forward=messages(&s.connection,1,1,None,2)?;assert_eq!(forward.items[0].content,"1");assert_eq!(forward.items[1].content,"2");
  assert!(recent_messages(&s.connection,1,1,None,0).is_err());assert!(recent_messages(&s.connection,2,1,None,100).is_err());Ok(())
 }).unwrap();
}
