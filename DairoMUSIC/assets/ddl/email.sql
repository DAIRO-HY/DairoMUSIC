-- 邮件发送记录
CREATE TABLE email
(
    id        INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, -- 主键
    fromEmail VARCHAR(128)  NOT NULL, -- 发件邮箱地址
    email     VARCHAR(128)  NOT NULL, -- 收件邮箱地址
    subject   VARCHAR(128), -- 标题
    content   VARCHAR(5000) NOT NULL, -- 邮件内容
    emailType VARCHAR(32)   NOT NULL, -- 邮件类型 LOGIN:登录验证码 REGIST:注册验证码 MIDIFY:修改密码验证码
    code      VARCHAR(16), -- 验证码
    state     INTEGER       NOT NULL default 0, -- 发送状态 0:未发送 1:发送成功 2:发送失败
    error     VARCHAR(512), -- 发送失败的原因
    sendTime  INT8, -- 发送时间(毫秒)
    date      INT8          NOT NULL -- 创建时间(毫秒)
);
CREATE INDEX idx_email_email ON email (email);
CREATE INDEX idx_email_type ON email (emailType);
