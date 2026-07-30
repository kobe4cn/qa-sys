use std::{collections::HashMap, ops::DerefMut};

use anyhow::{Context, Result, bail, ensure};
use chrono::Local;
use futures::TryStreamExt;
use pulsar::{Consumer, Pulsar, SubType, TokioExecutor, producer, proto};
use sqlx::PgPool;
use tokio::sync::watch;
use tracing::{error, info};

use crate::{
    AnswerEntity, UserVoteEntity, UserVoteRepository, VoteMessage,
    config::xpulsar::VoteMessagingConfig,
};

pub struct UserVoteRepositoryImpl {
    db: PgPool,
    mq: Pulsar<TokioExecutor>,
    messaging: VoteMessagingConfig,
}

impl UserVoteRepositoryImpl {
    pub fn new(db: PgPool, mq: Pulsar<TokioExecutor>, messaging: VoteMessagingConfig) -> Self {
        Self { db, mq, messaging }
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
        created_by: String,
        action: String,
    ) -> Result<bool> {
        let has_voted = self
            .is_voted(target_id, created_by.clone(), "answer".to_string())
            .await?;
        match action.as_str() {
            "up" => {
                if has_voted {
                    info!("user :{} has voted answer id:{}", created_by, target_id);
                    return Ok(false);
                }
                self.vote_answer(target_id, created_by).await
            }
            "down" => {
                if !has_voted {
                    info!("user:{} hasn't voted answer:【{}】", created_by, target_id);
                    return Ok(false);
                }
                self.cancel_vote_answer(target_id, created_by).await
            }
            _ => {
                bail!("unsupported answer vote action");
            }
        }
    }

    async fn vote_answer(&self, target_id: u64, created_by: String) -> Result<bool> {
        let target_id =
            i64::try_from(target_id).context("answer vote target ID exceeds PostgreSQL bigint")?;
        let now = Local::now().naive_local();
        let mut transaction = self.db.begin().await?;
        let insert_vote = format!(
            "insert into {} (target_id,target_type,created_by,created_at) values ($1,$2,$3,$4)",
            UserVoteEntity::table_name()
        );
        let vote_result = sqlx::query(sqlx::AssertSqlSafe(insert_vote))
            .bind(target_id)
            .bind("answer")
            .bind(created_by)
            .bind(now)
            .execute(transaction.deref_mut())
            .await?;
        ensure!(
            vote_result.rows_affected() == 1,
            "failed to insert answer vote"
        );

        let update_answer = format!(
            "update {} set agree_count = agree_count + $1, updated_at = $2 where id = $3",
            AnswerEntity::table_name(),
        );
        let answer_result = sqlx::query(sqlx::AssertSqlSafe(update_answer))
            .bind(1_i64)
            .bind(now)
            .bind(target_id)
            .execute(transaction.deref_mut())
            .await?;
        ensure!(
            answer_result.rows_affected() == 1,
            "answer vote target does not exist"
        );

        transaction.commit().await?;
        Ok(true)
    }

    async fn cancel_vote_answer(&self, target_id: u64, created_by: String) -> Result<bool> {
        let target_id =
            i64::try_from(target_id).context("answer vote target ID exceeds PostgreSQL bigint")?;
        let now = Local::now().naive_local();
        let mut transaction = self.db.begin().await?;
        let delete_vote = format!(
            "delete from {} where target_id = $1 and target_type = $2 and created_by = $3",
            UserVoteEntity::table_name(),
        );
        let vote_result = sqlx::query(sqlx::AssertSqlSafe(delete_vote))
            .bind(target_id)
            .bind("answer")
            .bind(created_by)
            .execute(transaction.deref_mut())
            .await?;
        ensure!(
            vote_result.rows_affected() == 1,
            "answer vote record does not exist"
        );

        let update_answer = format!(
            "update {} set agree_count = greatest(agree_count - $1, 0), updated_at = $2 where id \
             = $3",
            AnswerEntity::table_name(),
        );
        let answer_result = sqlx::query(sqlx::AssertSqlSafe(update_answer))
            .bind(1_i64)
            .bind(now)
            .bind(target_id)
            .execute(transaction.deref_mut())
            .await?;
        ensure!(
            answer_result.rows_affected() == 1,
            "answer vote target does not exist"
        );

        transaction.commit().await?;
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
            "select id from {} where target_id=$1 and created_by=$2 and target_type=$3",
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
        if target_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let lens = target_ids.len();
        let hold = self.gen_in_placeholder(target_ids.len());
        let sql = format!(
            "select target_id from {} where target_id in ({}) and target_type= ${} and \
             created_by=${}",
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
        let mut producer = self
            .mq
            .producer()
            .with_topic(self.messaging.topic.clone())
            .with_name(self.messaging.producer_name.clone())
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
        let receipt = producer.send_non_blocking(msg).await?;
        receipt.await?;
        Ok(true)
    }
    async fn consumer(
        &self,
        target_type: String,
        mut receive: watch::Receiver<bool>,
    ) -> Result<()> {
        let client = self.mq.clone();
        let mut consumer: Consumer<VoteMessage, _> = client
            .consumer()
            .with_topic(self.messaging.topic.clone())
            .with_subscription(self.messaging.subscription.clone())
            .with_subscription_type(SubType::Exclusive)
            .with_consumer_name(self.messaging.consumer_name.clone())
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
