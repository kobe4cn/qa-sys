-- Add migration script here
-- 1. 创建数据库


-- 在 psql 命令行中可执行: \c qa_sys 切换数据库

-- ========================================================
-- 2. 问题表 (questions)
-- ========================================================
CREATE TABLE questions (
    id bigserial PRIMARY KEY,
    title varchar(300) NOT NULL DEFAULT '',
    content text NOT NULL,
    created_by varchar(50) NOT NULL DEFAULT '',
    updated_by varchar(50) NOT NULL DEFAULT '',
    created_at timestamp NOT NULL,
    updated_at timestamp DEFAULT NULL,
    read_count bigint NOT NULL DEFAULT 0,
    reply_count bigint NOT NULL DEFAULT 0
);

-- 索引
CREATE INDEX idx_questions_created_by ON questions(created_by);
CREATE INDEX idx_questions_created_at ON questions(created_at);

-- 表及字段注释
COMMENT ON TABLE questions IS '问题表';
COMMENT ON COLUMN questions.id IS '自增id';
COMMENT ON COLUMN questions.title IS '标题';
COMMENT ON COLUMN questions.content IS '内容';
COMMENT ON COLUMN questions.created_by IS '创建者';
COMMENT ON COLUMN questions.updated_by IS '更新者';
COMMENT ON COLUMN questions.created_at IS '创建时间';
COMMENT ON COLUMN questions.updated_at IS '更新时间';
COMMENT ON COLUMN questions.read_count IS '阅读数';
COMMENT ON COLUMN questions.reply_count IS '回答数';


-- ========================================================
-- 3. 回答表 (answers)
-- ========================================================
CREATE TABLE answers (
    id bigserial PRIMARY KEY,
    question_id bigint NOT NULL DEFAULT 0,
    content text NOT NULL,
    created_by varchar(50) NOT NULL DEFAULT '',
    updated_by varchar(50) NOT NULL DEFAULT '',
    created_at timestamp NOT NULL,
    updated_at timestamp DEFAULT NULL,
    agree_count bigint NOT NULL DEFAULT 0
);

-- 索引
CREATE INDEX idx_answers_question_id ON answers(question_id);
CREATE INDEX idx_answers_created_by ON answers(created_by);
CREATE INDEX idx_answers_created_at ON answers(created_at);

-- 表及字段注释
COMMENT ON TABLE answers IS '回答表';
COMMENT ON COLUMN answers.id IS '自增id';
COMMENT ON COLUMN answers.question_id IS '问题id';
COMMENT ON COLUMN answers.content IS '内容';
COMMENT ON COLUMN answers.created_by IS '创建者';
COMMENT ON COLUMN answers.updated_by IS '更新者';
COMMENT ON COLUMN answers.created_at IS '创建时间';
COMMENT ON COLUMN answers.updated_at IS '更新时间';
COMMENT ON COLUMN answers.agree_count IS '点赞数';


-- ========================================================
-- 4. 用户表 (users)
-- ========================================================
CREATE TABLE users (
    id bigserial PRIMARY KEY,
    username varchar(50) NOT NULL,
    password varchar(50) NOT NULL,
    nick varchar(100) NOT NULL DEFAULT '',
    openid varchar(32) NOT NULL DEFAULT '',
    created_at timestamp NOT NULL,
    updated_at timestamp DEFAULT NULL,
    CONSTRAINT uk_users_username UNIQUE (username)
);

-- 索引
CREATE INDEX idx_users_openid ON users(openid);
CREATE INDEX idx_users_created_at ON users(created_at);

-- 表及字段注释
COMMENT ON TABLE users IS '用户表';
COMMENT ON COLUMN users.id IS '自增id';
COMMENT ON COLUMN users.username IS '用户名';
COMMENT ON COLUMN users.password IS '密码，用户登录和注册使用';
COMMENT ON COLUMN users.nick IS '用户昵称';
COMMENT ON COLUMN users.openid IS '用户openid';
COMMENT ON COLUMN users.created_at IS '创建时间';
COMMENT ON COLUMN users.updated_at IS '更新时间';


-- ========================================================
-- 5. 点赞纪录表 (users_votes)
-- ========================================================
CREATE TABLE users_votes (
    id bigserial PRIMARY KEY,
    target_id bigint NOT NULL DEFAULT 0,
    target_type varchar(50) NOT NULL DEFAULT '',
    created_by varchar(50) NOT NULL DEFAULT '',
    created_at timestamp NOT NULL
);

-- 索引
CREATE INDEX idx_users_votes_created_by ON users_votes(created_by);
CREATE INDEX idx_users_votes_target_id ON users_votes(target_id);

-- 表及字段注释
COMMENT ON TABLE users_votes IS '点赞纪录表';
COMMENT ON COLUMN users_votes.id IS '自增id';
COMMENT ON COLUMN users_votes.target_id IS '实体id';
COMMENT ON COLUMN users_votes.target_type IS '实体类型';
COMMENT ON COLUMN users_votes.created_by IS '创建者';
COMMENT ON COLUMN users_votes.created_at IS '创建时间';
