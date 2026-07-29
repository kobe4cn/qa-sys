use std::{collections::HashMap, ops::DerefMut};

use anyhow::Result;
use chrono::Local;
use futures::TryStreamExt;
use pulsar::{Consumer, Pulsar, SubType, TokioExecutor, producer, proto};

use sqlx::PgPool;
use tokio::sync::watch;
use tracing::{error, info};

use crate::{AnswerEntity, UserVoteEntity, UserVoteRepository, VoteMessage};

pub struct UserVoteRepositoryImpl {
    db: PgPool,
    mq: Pulsar<TokioExecutor>,
}

impl UserVoteRepositoryImpl {
    pub fn new(db: PgPool, mq: Pulsar<TokioExecutor>) -> Self {
        Self { db, mq }
    }
    pub fn gen_in_placeholder(&self, len: usize) -> String {
        (1..=len)
            .into_iter()
            .map(|i| format!("${i}"))
            .collect::<Vec<String>>()
            .join(",")
    }

    async fn handler_answer_agree(
        &self,
        target_id: u64,
        create_by: String,
        action: String,
    ) -> Result<bool> {
        let res = self
            .is_voted(target_id, create_by.clone(), action.clone())
            .await;
        if action == "up" {
            if res.is_ok() {
                info!("user :{} has voted answer id:{}", create_by, target_id);
                return Ok(false);
            }
            self.vote_answer(target_id, create_by).await
        } else {
            let has_voted = res.unwrap_or(false);
            if !has_voted {
                info!("user:{} hasn't voted answer:【{}】", create_by, target_id);
                return Ok(false);
            }
            self.cancel_vote_answer(target_id, create_by).await
        }
    }

    async fn vote_answer(&self, target_id: u64, create_by: String) -> Result<bool> {
        let sql = format!(
            r#"insert into {} (target_id,target_type,create_by,create_at) values ($1,$2,$3,$4)"#,
            UserVoteEntity::table_name()
        );
        let mut tx = self.db.begin().await?;
        let aw = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(target_id as i64)
            .bind("answer")
            .bind(create_by)
            .bind(Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
            .execute(tx.deref_mut())
            .await?;
        info!("vote rows_affected: {}", aw.rows_affected());
        let sql = format!(
            r#"update {} set agree_count = agree_count + ?,updated_at = ? where id = ?"#,
            AnswerEntity::table_name(),
        );
        let aw1 = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(1 as i64)
            .bind(Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
            .bind(target_id as i64)
            .execute(tx.deref_mut())
            .await?;
        info!("vote answer rows_affected: {}", aw1.rows_affected());
        tx.commit().await?;
        Ok(true)
    }

    async fn cancel_vote_answer(&self, target_id: u64, create_by: String) -> Result<bool> {
        // 删除点赞记录
        let sql = format!(
            r#"delete from {} where target_id = ? and target_type = ? and created_by = ?"#,
            UserVoteEntity::table_name(),
        );
        println!("cancel vote sql:{}", sql);
        let mysql_pool = &self.db;
        let mut tx = mysql_pool.begin().await?;
        // 删除点赞明细记录
        let affect_res = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(target_id as i64)
            .bind("answer")
            .bind(create_by)
            // 在sqlx 0.7版本以上，execute这里需要对tx进行解引用并获取内部DB的可变引用connection
            .execute(tx.deref_mut())
            .await?;
        info!("cancel vote affect_rows:{}", affect_res.rows_affected());

        // 查询回答点赞数
        let sql = format!(
            "select id,agree_count from {} where id = ?",
            AnswerEntity::table_name(),
        );
        let res: (i64, i64) = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(target_id as i64)
            .fetch_one(&self.db)
            .await?;
        let agree_count = res.1; // 当前回答点赞数
        let mut remain = agree_count as i64 - 1; // 取消点赞后的点赞数
        if remain <= 0 {
            info!(
                "current answer id:{} agree_count:{} remain:{}",
                target_id, agree_count, remain
            );
            remain = 0;
        }

        // 更新当前问题点赞数
        let sql = format!(
            r#"update {} set agree_count = ?,updated_at = ? where id = ?"#,
            AnswerEntity::table_name(),
        );
        let updated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        println!("cancel vote update sql:{}", sql);
        let affect_res = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(remain)
            .bind(updated_at)
            .bind(target_id as i64)
            .execute(tx.deref_mut())
            .await?;
        info!(
            "update answer vote affect_rows:{}",
            affect_res.rows_affected()
        );
        // 提交事务
        tx.commit().await?;
        Ok(true)
    }
}

#[async_trait::async_trait]
impl UserVoteRepository for UserVoteRepositoryImpl {
    async fn is_voted(
        &self,
        target_id: u64,
        username: String,
        target_type: String,
    ) -> Result<bool> {
        let sql = format!(
            "select id from {} where target_id=$1 and create_by=$2 and target_type=$3",
            UserVoteEntity::table_name()
        );
        let result: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(target_id as i64)
            .bind(username)
            .bind(target_type)
            .fetch_optional(&self.db)
            .await?;
        Ok(result.is_some())
    }
    async fn is_batch_voted(
        &self,
        target_ids: Vec<u64>,
        username: String,
        target_type: String,
    ) -> Result<HashMap<u64, bool>> {
        let lens = target_ids.len();
        let hold = self.gen_in_placeholder(target_ids.len());
        let sql = format!(
            "select target_id from {} where target_id in ({}) and target_type= ${} and create_by=${}",
            UserVoteEntity::table_name(),
            hold,
            lens + 1,
            lens + 2,
        );

        let mut q = sqlx::query_as(sqlx::AssertSqlSafe(sql));

        for i in &target_ids {
            q = q.bind(*i as i64);
        }
        let rest: Vec<(i64,)> = q
            .bind(target_type)
            .bind(username)
            .fetch_all(&self.db)
            .await?;
        let mut result: HashMap<u64, bool> = HashMap::with_capacity(rest.len() + 1);
        for (id,) in rest {
            result.insert(id as u64, true);
        }
        Ok(result)
    }
    async fn publish(&self, msg: VoteMessage) -> Result<bool> {
        let topic = "user-vote-topic";
        let mut producer = self
            .mq
            .producer()
            .with_topic(topic)
            .with_name("qa-sys")
            .with_options(producer::ProducerOptions {
                schema: Some(proto::Schema {
                    r#type: proto::schema::Type::String as i32,
                    ..Default::default()
                }),
                ..Default::default()
            })
            .build()
            .await?;
        producer.check_connection().await?;
        let res = producer.send_non_blocking(msg).await;
        Ok(res.is_ok())
    }
    async fn consumer(
        &self,
        target_type: String,
        mut receive: watch::Receiver<bool>,
    ) -> Result<()> {
        let topic = "user-vote-topic";
        let client = self.mq.clone();
        let mut consumer: Consumer<VoteMessage, _> = client
            .consumer()
            .with_topic(topic)
            .with_subscription("qa-sys")
            .with_subscription_type(SubType::Exclusive)
            .with_consumer_name("group-1")
            .build()
            .await?;
        let mut counter: usize = 0;
        loop {
            tokio::select! {
                changed = receive.changed() =>{
                    if changed.is_err(){
                        info!("receive shutdown signal!");
                        break;
                    }

                }
                res = consumer.try_next()=>{
                    let msg=match res?{
                        Some(msg) => msg,
                        None=> break,
                    };
                    info!("receive msg={:#?}",msg);
                    let data=match msg.deserialize(){
                        Ok(data)=>data,
                        Err(e)=>{
                            error!("msg=parse err={:?}",e);
                            continue;
                        }
                    };

                    if target_type == "answer" {
                        let reply = self.handler_answer_agree(data.target_id as u64, data.created_by, data.action).await;
                        if reply.is_err(){
                            error!("answer agree reply={:?}", reply);
                            continue;
                        }
                        info!("answer agree reply={:?}", reply);
                    }
                    consumer.ack(&msg).await?;

                    counter += 1;
                    info!("counter={}", counter);
                }
            }
        }
        Ok(())
    }
}
