use std::collections::HashMap;

use autometrics::{
    autometrics,
    objectives::{Objective, ObjectiveLatency, ObjectivePercentile},
};
use chrono::{Local, TimeZone};
use pb::{UserRegisterResponse, qa_service_server::QaService};
use qa_sys_core::{AesCBCCrypto, AesKeySize};
use tonic::{
    Code::{self},
    Status,
};
use tracing::info;
use uuid::Uuid;

use crate::{
    APP_CONFIG, AnswerEntity, AnswerRepository, AnswerRepositoryImpl, AppState, QuestionEntity,
    QuestionRepository, QuestionRepositoryImpl, UserRepository, UserRepositoryImpl,
    UserSessionEntity, UserVoteRepository, UserVoteRepositoryImpl, VoteMessage,
};

const API_SLO: Objective = Objective::new("grpc")
    // We expect 99.9% of all requests to succeed.
    .success_rate(ObjectivePercentile::P99_9)
    // We expect 99% of all latencies to be below 750ms.
    .latency(ObjectiveLatency::Ms750, ObjectivePercentile::P99);

pub struct QaServiceImpl {
    answer_repo: Box<dyn AnswerRepository>,
    question_repo: Box<dyn QuestionRepository>,
    user_vote_repo: Box<dyn UserVoteRepository>,
    user_repo: Box<dyn UserRepository>,
    aes_crypto: AesCBCCrypto,
}

impl QaServiceImpl {
    pub fn new(state: AppState) -> Self {
        let answer_repo = Box::new(AnswerRepositoryImpl::new(state.pgsql_pool.clone()));
        let question_repo = Box::new(QuestionRepositoryImpl::new(
            state.pgsql_pool.clone(),
            state.redis_pool.clone(),
        ));
        let user_vote_repo = Box::new(UserVoteRepositoryImpl::new(
            state.pgsql_pool.clone(),
            state.pulsar_client.clone(),
            APP_CONFIG.vote_messaging_conf.clone(),
        ));
        let user_repo = Box::new(UserRepositoryImpl::new(
            state.pgsql_pool.clone(),
            state.redis_pool.clone(),
        ));
        let aes_crypto =
            AesCBCCrypto::new(&APP_CONFIG.aes_key, &APP_CONFIG.aes_iv, AesKeySize::Size256)
                .expect("fail init aes crypto");
        Self {
            answer_repo,
            question_repo,
            user_vote_repo,
            user_repo,
            aes_crypto,
        }
    }
    // 验证token是否有效，返回用户唯一标识openid
    fn check_token(&self, token: &str) -> Result<String, String> {
        if token.is_empty() {
            return Err("token length invalid".to_string());
        }

        let payload = self.aes_crypto.decrypt(token).map_err(|error| {
            info!("failed to decrypt authentication token, error:{error:?}");
            format!("parse token error:{error:?}")
        })?;
        let mut parts = payload.split(':');
        let Some(_login_id) = parts.next() else {
            return Err("token invalid".to_string());
        };
        let Some(openid) = parts.next() else {
            return Err("token invalid".to_string());
        };
        let Some(expire_time) = parts.next() else {
            return Err("token invalid".to_string());
        };
        if parts.next().is_some() {
            return Err("token invalid".to_string());
        }
        if openid.len() != 32 {
            return Err("token length invalid".to_string());
        }

        let expire_time = expire_time
            .parse::<i64>()
            .map_err(|error| format!("token expire_time parse error:{error}"))?;
        let current_time = Local::now().timestamp();
        if current_time >= expire_time {
            return Err("token has expired".to_string());
        }

        Ok(openid.to_string())
    }
}

#[async_trait::async_trait]
impl QaService for QaServiceImpl {
    #[autometrics(objective = API_SLO)]
    async fn user_login(
        &self,
        request: tonic::Request<pb::UserLoginRequest>,
    ) -> std::result::Result<tonic::Response<pb::UserLoginResponse>, tonic::Status> {
        let req = request.into_inner();
        let user = self.user_repo.fetch_one(req.username.clone()).await;
        match user {
            Ok(u) => {
                let pwd = format!("{:x}", md5::compute(req.password.as_bytes()));
                if u.password != pwd {
                    return Err(Status::new(
                        Code::InvalidArgument,
                        format!("用户 {} 输入的密码错误", req.username),
                    ));
                }

                let key = format!("user_login:{}", u.openid);
                let res = self.user_repo.get(key.clone()).await;
                if res.is_ok() {
                    return Err(Status::new(
                        Code::AlreadyExists,
                        format!("用户 {} 已经登陆，请不要重复登录", req.username),
                    ));
                }
                let login_id = Uuid::new_v4().to_string().replace("-", "");
                let login_time = Local::now();
                let expired = login_time.timestamp() + 86400;
                let payload = format!("{}:{}:{}", login_id, u.openid, expired);
                let token = self
                    .aes_crypto
                    .encrypt(&payload)
                    .expect("fail to encrypt token");
                let expire_time = Local.timestamp_opt(expired, 0).unwrap();
                let user_session = UserSessionEntity {
                    uid: u.id,
                    username: u.username,
                    openid: u.openid,
                    login_time: login_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                    expire_time: expire_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                };
                if let Err(e) = self.user_repo.set(key, user_session, 86400).await {
                    return Err(Status::new(
                        Code::Internal,
                        format!("保存 Session 失败: {}", e),
                    ));
                }
                Ok(tonic::Response::new(pb::UserLoginResponse { token }))
            }
            Err(err) => {
                let err = err.downcast().expect("fail to conver into sqlx error");
                match err {
                    sqlx::Error::RowNotFound => Err(Status::new(
                        Code::Unknown,
                        format!("当前用户 {} 未注册", req.username),
                    )),
                    other => Err(Status::new(
                        Code::Internal,
                        format!("用户 {} 登陆发生未知错误: {}", req.username, other),
                    )),
                }
            }
        }
    }
    #[autometrics(objective = API_SLO)]
    async fn user_logout(
        &self,
        request: tonic::Request<pb::UserLogoutRequest>,
    ) -> std::result::Result<tonic::Response<pb::UserLogoutResponse>, tonic::Status> {
        let res = request.into_inner();
        let login_res = self.check_token(&res.token);
        if let Err(err) = login_res {
            if err.to_string().contains("token has expired") {
                return Err(Status::new(
                    Code::Unauthenticated,
                    "token has expired".to_string(),
                ));
            }
            return Err(Status::new(Code::InvalidArgument, err));
        }
        let openid = login_res.unwrap();
        let key = format!("user_login:{}", openid);
        let res = self.user_repo.del(key).await;
        if let Err(err) = res {
            return Err(Status::new(
                Code::Aborted,
                format!("用户登出 失败:{:?}", err),
            ));
        }
        Ok(tonic::Response::new(pb::UserLogoutResponse { state: 1 }))
    }
    async fn user_register(
        &self,
        request: tonic::Request<pb::UserRegisterRequest>,
    ) -> std::result::Result<tonic::Response<pb::UserRegisterResponse>, tonic::Status> {
        let req = request.into_inner();
        let exists = self
            .user_repo
            .check_user_exist(req.username.clone())
            .await
            .map_err(|error| {
                Status::new(
                    Code::Internal,
                    format!("查询用户 {} 失败:{}", req.username, error),
                )
            })?;
        if exists {
            return Err(Status::new(Code::AlreadyExists, "用户名已存在"));
        }

        if let Err(error) = self.user_repo.add(req.username.clone(), req.password).await {
            return Err(Status::new(
                Code::Unknown,
                format!("用户 {} 注册失败:{}", req.username, error),
            ));
        }
        let reply = UserRegisterResponse { state: 1 };
        Ok(tonic::Response::new(reply))
    }

    #[autometrics(objective = API_SLO)]
    async fn verify_token(
        &self,
        request: tonic::Request<pb::VerifyTokenRequest>,
    ) -> std::result::Result<tonic::Response<pb::VerifyTokenResponse>, tonic::Status> {
        let req = request.into_inner();
        let login_res = self.check_token(&req.token);
        if let Err(err) = login_res {
            if err.to_string().contains("token has expired") {
                return Err(Status::new(
                    Code::Unauthenticated,
                    "token has expired".to_string(),
                ));
            }
            return Err(Status::new(Code::InvalidArgument, err));
        }
        let openid = login_res.unwrap();
        let res = self.user_repo.get(format!("user_login:{}", openid)).await;
        if let Err(err) = res {
            if err.to_string().contains("session not found") {
                let reply = pb::VerifyTokenResponse {
                    state: 0,
                    reason: "login session not found".to_string(),
                    username: "".to_string(),
                };
                return Ok(tonic::Response::new(reply));
            }
            let reply = pb::VerifyTokenResponse {
                state: 1,
                reason: format!("Unkown error: {}", err),
                username: "".to_string(),
            };
            return Ok(tonic::Response::new(reply));
        }
        let user = res.unwrap();
        let reply = pb::VerifyTokenResponse {
            state: 1,
            reason: "".to_string(),
            username: user.username,
        };
        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn add_question(
        &self,
        request: tonic::Request<pb::AddQuestionRequest>,
    ) -> std::result::Result<tonic::Response<pb::AddQuestionResponse>, tonic::Status> {
        let req = request.into_inner();
        let question = QuestionEntity {
            title: req.title,
            content: req.content,
            created_by: req.create_by,
            ..Default::default()
        };
        let res = self.question_repo.add(question).await;
        if let Err(err) = res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to add question, error:{}", err),
            ));
        }
        let reply = pb::AddQuestionResponse {
            id: res.unwrap() as u64,
        };
        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn delete_question(
        &self,
        request: tonic::Request<pb::DeleteQuestionRequest>,
    ) -> std::result::Result<tonic::Response<pb::DeleteQuestionResponse>, tonic::Status> {
        let req = request.into_inner();
        let res = self.question_repo.delete(req.id, req.username).await;
        if let Err(err) = res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to delete question, error:{}", err),
            ));
        }
        let reply = pb::DeleteQuestionResponse { state: 1 };
        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn update_question(
        &self,
        request: tonic::Request<pb::UpdateQuestionRequest>,
    ) -> std::result::Result<tonic::Response<pb::UpdateQuestionResponse>, tonic::Status> {
        let req = request.into_inner();
        let question = QuestionEntity {
            title: req.title,
            content: req.content,
            updated_by: req.update_by,
            ..Default::default()
        };
        let res = self.question_repo.update(req.id, question).await;
        if let Err(err) = res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to update question, error:{}", err),
            ));
        }
        let reply = pb::UpdateQuestionResponse { state: 1 };
        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn add_answer(
        &self,
        request: tonic::Request<pb::AddAnswerRequest>,
    ) -> std::result::Result<tonic::Response<pb::AddAnswerResponse>, tonic::Status> {
        let req = request.into_inner();
        if req.answer.is_none() {
            return Err(Status::new(Code::InvalidArgument, "answer is empty"));
        }
        let answer = req.answer.unwrap();
        let answer = AnswerEntity {
            question_id: answer.question_id as i64,
            content: answer.content,
            created_by: answer.create_by,
            ..Default::default()
        };
        let res = self.answer_repo.add(answer).await;
        if let Err(err) = res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to add answer, error:{}", err),
            ));
        }
        let reply = pb::AddAnswerResponse {
            id: res.unwrap() as u64,
        };
        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn delete_answer(
        &self,
        request: tonic::Request<pb::DeleteAnswerRequest>,
    ) -> std::result::Result<tonic::Response<pb::DeleteAnswerResponse>, tonic::Status> {
        let req = request.into_inner();
        let res = self.answer_repo.delete(req.id, req.username).await;
        if let Err(err) = res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to delete answer, error:{}", err),
            ));
        }
        let reply = pb::DeleteAnswerResponse { state: 1 };
        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn update_answer(
        &self,
        request: tonic::Request<pb::UpdateAnswerRequest>,
    ) -> std::result::Result<tonic::Response<pb::UpdateAnswerResponse>, tonic::Status> {
        let req = request.into_inner();
        let res = self
            .answer_repo
            .update(req.id, req.content, req.username)
            .await;
        if let Err(err) = res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to update answer, error:{}", err),
            ));
        }
        let reply = pb::UpdateAnswerResponse { state: 1 };
        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn agree_answer(
        &self,
        request: tonic::Request<pb::AgreeAnswerRequest>,
    ) -> std::result::Result<tonic::Response<pb::AgreeAnswerResponse>, tonic::Status> {
        let req = request.into_inner();
        let answer_res = self.answer_repo.find_one(req.id).await;
        if let Err(err) = answer_res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to find answer, error:{}", err),
            ));
        }
        let is_voted = self
            .user_vote_repo
            .is_voted(req.id, req.create_by.clone(), "answer".to_string())
            .await
            .unwrap_or(false);
        if req.action == "up" {
            if is_voted {
                let reply = pb::AgreeAnswerResponse {
                    state: 0,
                    agree_count: 0,
                    reason: "already voted it".to_string(),
                };
                return Ok(tonic::Response::new(reply));
            }
        } else {
            if !is_voted {
                let reply = pb::AgreeAnswerResponse {
                    state: 0,
                    agree_count: 0,
                    reason: "not voted it".to_string(),
                };
                return Ok(tonic::Response::new(reply));
            }
        }
        let mut agree_count = answer_res.unwrap().agree_count;
        if req.action == "up" {
            agree_count += 1;
        } else {
            agree_count -= 1;
        }

        let msg = VoteMessage {
            target_id: req.id as i64,
            target_type: "answer".to_string(),
            created_by: req.create_by.clone(),
            action: req.action,
        };
        let res = self.user_vote_repo.publish(msg).await;
        if let Err(e) = res {
            return Err(Status::new(
                Code::Internal,
                format!("failed to publish vote message, error:{}", e),
            ));
        }
        let reply = pb::AgreeAnswerResponse {
            state: 1,
            agree_count: agree_count as u64,
            reason: "".to_string(),
        };

        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn question_detail(
        &self,
        request: tonic::Request<pb::QuestionDetailRequest>,
    ) -> std::result::Result<tonic::Response<pb::QuestionDetailResponse>, tonic::Status> {
        let req = request.into_inner();
        let res = self.question_repo.find_one(req.id).await;
        if let Err(err) = res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to find question, error:{}", err),
            ));
        }
        let question = res.unwrap();
        let mut read_count = question.read_count;
        let view_count = self
            .question_repo
            .incr(question.id as u64, "question".to_string())
            .await;
        if let Err(err) = view_count {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to incr question, error:{}", err),
            ));
        } else {
            read_count += view_count.unwrap() as i64;
        }
        let question = pb::QuestionEntity {
            id: question.id as u64,
            title: question.title,
            content: question.content,
            read_count: read_count as u64,
            create_by: question.created_by,
            reply_count: question.reply_count as u64,
        };

        let reply = pb::QuestionDetailResponse {
            question: Some(question),
        };
        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn latest_question(
        &self,
        request: tonic::Request<pb::LatestQuestionRequest>,
    ) -> std::result::Result<tonic::Response<pb::LatestQuestionResponse>, tonic::Status> {
        let req = request.into_inner();
        let question_res = self.question_repo.find_latest(req.last_id, req.limit).await;
        if let Err(err) = question_res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to find latest question, error:{}", err),
            ));
        }
        let result = question_res.unwrap();
        if result.questions.is_empty() {
            let reply = pb::LatestQuestionResponse {
                last_id: 0,
                is_end: true,
                questions: vec![],
            };
            return Ok(tonic::Response::new(reply));
        }

        let question_list: Vec<pb::QuestionEntity> = result
            .questions
            .iter()
            .map(|q| pb::QuestionEntity {
                id: q.id as u64,
                title: q.title.clone(),
                content: q.content.clone(),
                read_count: q.read_count as u64,
                create_by: q.created_by.clone(),
                reply_count: q.reply_count as u64,
            })
            .collect();
        let reply = pb::LatestQuestionResponse {
            last_id: result.last_id.unwrap_or(0) as u64,
            is_end: result.is_end,
            questions: question_list,
        };
        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn answer_list(
        &self,
        request: tonic::Request<pb::AnswerListRequest>,
    ) -> std::result::Result<tonic::Response<pb::AnswerListResponse>, tonic::Status> {
        let req = request.into_inner();
        let res = self
            .answer_repo
            .find_latest(req.question_id, req.limit, req.page)
            .await;
        if let Err(err) = res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to find answer, error:{}", err),
            ));
        }
        let res = res.unwrap();
        if res.total == 0 {
            let reply = pb::AnswerListResponse {
                total: 0,
                total_page: 0,
                page_size: 0,
                current_page: 0,
                answers: vec![],
                is_end: true,
            };
            return Ok(tonic::Response::new(reply));
        }

        let ids = res.answers.iter().map(|a| a.id as u64).collect();
        let vote_map = self
            .user_vote_repo
            .is_batch_voted(ids, req.username, "answer".to_string())
            .await
            .unwrap_or(HashMap::default());

        let answers = res
            .answers
            .iter()
            .map(|a| pb::AnswerEntity {
                id: a.id as u64,
                question_id: a.question_id as u64,
                content: a.content.clone(),
                create_by: a.created_by.clone(),
                agree_count: a.agree_count as u64,
                has_aggreed: vote_map.contains_key(&(a.id as u64)),
            })
            .collect();

        let reply = pb::AnswerListResponse {
            total: res.total as u64,
            total_page: res.total_page as u64,
            page_size: res.page_size as u64,
            current_page: res.current_page as u64,
            answers,
            is_end: res.is_end,
        };
        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn answer_detail(
        &self,
        request: tonic::Request<pb::AnswerDetailRequest>,
    ) -> std::result::Result<tonic::Response<pb::AnswerDetailResponse>, tonic::Status> {
        let req = request.into_inner();
        let res = self.answer_repo.find_one(req.id).await;
        if let Err(err) = res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to find answer, error:{}", err),
            ));
        }
        let answer = res.unwrap();
        let vote_map = self
            .user_vote_repo
            .is_voted(answer.id as u64, req.username, "answer".to_string())
            .await
            .unwrap_or(false);
        let answer = pb::AnswerEntity {
            id: answer.id as u64,
            question_id: answer.question_id as u64,
            content: answer.content.clone(),
            create_by: answer.created_by.clone(),
            agree_count: answer.agree_count as u64,
            has_aggreed: vote_map,
        };
        let reply = pb::AnswerDetailResponse {
            answer: Some(answer),
        };
        Ok(tonic::Response::new(reply))
    }
    #[autometrics(objective = API_SLO)]
    async fn answer_agree(
        &self,
        request: tonic::Request<pb::AgreeAnswerRequest>,
    ) -> std::result::Result<tonic::Response<pb::AgreeAnswerResponse>, tonic::Status> {
        let req = request.into_inner();
        let answer_res = self.answer_repo.find_one(req.id).await;
        if let Err(err) = answer_res {
            return Err(Status::new(
                Code::Unknown,
                format!("failed to find answer, error:{}", err),
            ));
        }
        let is_voted = self
            .user_vote_repo
            .is_voted(req.id, req.create_by.clone(), "answer".to_string())
            .await
            .unwrap_or(false);
        if req.action == "up" {
            if is_voted {
                let reply = pb::AgreeAnswerResponse {
                    state: 0,
                    agree_count: 0,
                    reason: "already voted it".to_string(),
                };
                return Ok(tonic::Response::new(reply));
            }
        } else {
            if !is_voted {
                let reply = pb::AgreeAnswerResponse {
                    state: 0,
                    agree_count: 0,
                    reason: "not voted it".to_string(),
                };
                return Ok(tonic::Response::new(reply));
            }
        }
        let mut agree_count = answer_res.unwrap().agree_count;
        if req.action == "up" {
            agree_count += 1;
        } else {
            agree_count -= 1;
        }

        let msg = VoteMessage {
            target_id: req.id as i64,
            target_type: "answer".to_string(),
            created_by: req.create_by.clone(),
            action: req.action,
        };
        let res = self.user_vote_repo.publish(msg).await;
        if let Err(e) = res {
            return Err(Status::new(
                Code::Internal,
                format!("failed to publish vote message, error:{}", e),
            ));
        }
        let reply = pb::AgreeAnswerResponse {
            state: 1,
            agree_count: agree_count as u64,
            reason: "".to_string(),
        };

        Ok(tonic::Response::new(reply))
    }
}
