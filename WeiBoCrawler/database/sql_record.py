from datetime import datetime
from sqlalchemy import BigInteger, JSON, Text
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship

# 定义抽象基类
class AbstractBase(DeclarativeBase):
    __abstract__ = True
    id: Mapped[int] = mapped_column(primary_key=True)
    mid: Mapped[int] = mapped_column(BigInteger)
    uid: Mapped[int] = mapped_column(BigInteger)
    search_for: Mapped[str] = mapped_column(Text)
    create_time: Mapped[datetime] = mapped_column(default=lambda: datetime.now())
    json_data: Mapped[dict] = mapped_column(JSON)


class ListRecord(AbstractBase):
    """存储 List Record 的数据

    """
    __tablename__ = 'ListRecord' 

    def __repr__(self):
        return f"ListRecord(id={self.id}, mid={self.mid}, uid={self.uid}, search_for='{self.search_for}', create_time={self.create_time})"
    
class BodyRecord(AbstractBase):
    """存储 Body Record 的数据

    """
    __tablename__ = 'BodyRecord'

    def __repr__(self):
        return f"BodyRecord(id={self.id}, mid={self.mid}, uid={self.uid}, search_for='{self.search_for}', create_time={self.create_time}))"

class Comment1Record(AbstractBase):
    """存储 Comment Record 的数据

        """
    __tablename__ = 'Comment1Record'
    f_mid: Mapped[int] = mapped_column(BigInteger)  # 与主表字段类型一致
    f_uid: Mapped[int] = mapped_column(BigInteger)

    def __repr__(self):
        return f"CommentRecord(id={self.id}, mid={self.mid}, uid={self.uid}, f_mid={self.f_mid}, f_uid={self.f_uid}), search_for='{self.search_for}')"
    

class Comment2Record(AbstractBase):
    """存储 Comment Record 的数据

        """
    __tablename__ = 'Comment2Record'
    f_mid: Mapped[int] = mapped_column(BigInteger)  # 与主表字段类型一致
    f_uid: Mapped[int] = mapped_column(BigInteger)

    def __repr__(self):
        return f"CommentRecord(id={self.id}, mid={self.mid}, uid={self.uid}, f_mid={self.f_mid}, f_uid={self.f_uid}), search_for='{self.search_for}')"