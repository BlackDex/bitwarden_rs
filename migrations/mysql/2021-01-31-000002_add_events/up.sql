CREATE TABLE event (
  uuid              CHAR(36)    NOT NULL PRIMARY KEY,
  event_type        INTEGER     NOT NULL,
  user_uuid         CHAR(36),
  org_uuid          CHAR(36),
  cipher_uuid       CHAR(36),
  collection_uuid   CHAR(36),
  group_uuid        CHAR(36),
  org_user_uuid     CHAR(36),
  act_user_uuid     CHAR(36),
  device_type       INTEGER,
  ip_address        TEXT,
  event_date        DATETIME    NOT NULL,

  UNIQUE (uuid)
);