import enum
from datetime import datetime
from sqlalchemy import BigInteger, JSON, Text, ForeignKey, Enum
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship


class RecordFrom(enum.Enum):
    """主要在 BodyRecord 中使用，表示数据来源

    """
    Html = "html"
    Api = "api"


class Base(DeclarativeBase):
    """初始化 registry 属性

    """
    ...

class AbstractBase(Base):
    __abstract__ = True
    id: Mapped[int] = mapped_column(primary_key=True)
    mid: Mapped[int] = mapped_column(BigInteger)
    uid: Mapped[int] = mapped_column(BigInteger)
    search_for: Mapped[str] = mapped_column(Text)
    create_time: Mapped[datetime] = mapped_column(default=lambda: datetime.now())
    json_data: Mapped[dict] = mapped_column(JSON)


class BodyComment1(Base):
    """定义 BodyRecord 与 Comment1Record 的关联表

    """
    __tablename__ = 'body_comment1_association'
    id: Mapped[int] = mapped_column(primary_key=True)
    body_mid: Mapped[int] = mapped_column(BigInteger, ForeignKey('BodyRecord.mid'))
    body_uid: Mapped[int] = mapped_column(BigInteger, ForeignKey('BodyRecord.uid'))
    comment1_f_mid: Mapped[int] = mapped_column(BigInteger, ForeignKey('Comment1Record.mid'))
    comment1_f_uid: Mapped[int] = mapped_column(BigInteger, ForeignKey('Comment1Record.uid'))


class BodyComment2(Base):
    """定义 BodyRecord 与 Comment2Record 的关联表

    """
    __tablename__ = 'body_comment2_association'
    id: Mapped[int] = mapped_column(primary_key=True)
    body_uid: Mapped[int] = mapped_column(BigInteger, ForeignKey('BodyRecord.uid'))
    comment2_f_uid: Mapped[int] = mapped_column(BigInteger, ForeignKey('Comment2Record.f_uid'))


class Comment12(Base):
    """定义 Comment1Record 与 Comment2Record 的关联表

    """
    __tablename__ = 'comment1_comment2_association'
    id: Mapped[int] = mapped_column(primary_key=True)
    comment1_mid: Mapped[int] = mapped_column(BigInteger, ForeignKey('Comment1Record.mid'))
    comment2_f_mid: Mapped[int] = mapped_column(BigInteger, ForeignKey('Comment2Record.f_mid'))


class BodyRecord(AbstractBase):
    """存储 Body Record 的数据

    """
    __tablename__ = 'BodyRecord'
    record_from: Mapped[RecordFrom] = mapped_column(Enum(RecordFrom))

    # 定义关系字段
    comment1_records: Mapped[list["Comment1Record"]] = relationship(
        lazy=True,
        secondary="body_comment1_association",
        back_populates='body_records',
        primaryjoin="and_(BodyRecord.mid == body_comment1_association.c.body_mid, BodyRecord.uid == body_comment1_association.c.body_uid)",
        secondaryjoin="and_(Comment1Record.f_mid == body_comment1_association.c.comment1_f_mid, Comment1Record.f_uid == body_comment1_association.c.comment1_f_uid)",
        # cascade="all, delete-orphan", # 这里的 cascade 选项表示当 BodyRecord 被删除时，相关联的 Comment1Record 和 Comment2Record 也会被删除 ！！！多对多禁止使用
    )
    comment2_records: Mapped[list["Comment2Record"]] = relationship(
        lazy=True,
        secondary="body_comment2_association",
        back_populates='body_records',
        primaryjoin="BodyRecord.uid == body_comment2_association.c.body_uid",
        secondaryjoin="Comment2Record.f_uid == body_comment2_association.c.comment2_f_uid",
        # cascade="all, delete-orphan", # 这里的 cascade 选项表示当 BodyRecord 被删除时，相关联的 Comment1Record 和 Comment2Record 也会被删除 ！！！多对多禁止使用
    )

    def __repr__(self):
        return f"BodyRecord(id={self.id}, mid={self.mid}, uid={self.uid}, search_for='{self.search_for}', record_from='{self.record_from}', create_time={self.create_time})"


class Comment1Record(AbstractBase):
    """存储 Comment Record 的数据

    """
    __tablename__ = 'Comment1Record'
    f_mid: Mapped[int] = mapped_column(BigInteger)
    f_uid: Mapped[int] = mapped_column(BigInteger)

    # 定义关系字段
    body_records: Mapped[list["BodyRecord"]] = relationship(
        secondary="body_comment1_association",
        back_populates='comment1_records',
        primaryjoin="and_(Comment1Record.f_mid == body_comment1_association.c.comment1_f_mid, Comment1Record.f_uid == body_comment1_association.c.comment1_f_uid)",
        secondaryjoin="and_(BodyRecord.mid == body_comment1_association.c.body_mid, BodyRecord.uid == body_comment1_association.c.body_uid)"
    )
    comment2_records: Mapped[list["Comment2Record"]] = relationship(
        secondary="comment1_comment2_association",
        back_populates='comment1_records',
        primaryjoin="Comment1Record.mid == comment1_comment2_association.c.comment1_mid",
        secondaryjoin="Comment2Record.f_mid == comment1_comment2_association.c.comment2_f_mid"
    )

    def __repr__(self):
        return f"Comment1Record(id={self.id}, mid={self.mid}, uid={self.uid}, f_mid={self.f_mid}, f_uid={self.f_uid}, search_for='{self.search_for}')"


class Comment2Record(AbstractBase):
    """存储 Comment Record 的数据

    """
    __tablename__ = 'Comment2Record'
    f_mid: Mapped[int] = mapped_column(BigInteger)
    f_uid: Mapped[int] = mapped_column(BigInteger)

    # 定义关系字段
    body_records: Mapped[list["BodyRecord"]] = relationship(
        secondary="body_comment2_association",
        back_populates='comment2_records',
        primaryjoin="Comment2Record.f_uid == body_comment2_association.c.comment2_f_uid",
        secondaryjoin="BodyRecord.uid == body_comment2_association.c.body_uid"
    )
    comment1_records: Mapped[list["Comment1Record"]] = relationship(
        secondary="comment1_comment2_association",
        back_populates='comment2_records',
        primaryjoin="Comment2Record.f_mid == comment1_comment2_association.c.comment2_f_mid",
        secondaryjoin="Comment1Record.mid == comment1_comment2_association.c.comment1_mid"
    )

    def __repr__(self):
        return f"Comment2Record(id={self.id}, mid={self.mid}, uid={self.uid}, f_mid={self.f_mid}, f_uid={self.f_uid}, search_for='{self.search_for}')"
