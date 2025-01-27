import re
import parsel
import pandas as pd
from datetime import datetime
from .util import process_time_str
from typing import Optional, List
from ..type import custom_validate_call


@custom_validate_call
def get_mid(select: parsel.Selector) -> Optional[str]:
    """获取微博的mid

    Args:
        select (parsel.Selector): 经过 parsel 解析 html 后得到的 Selector 对象

    Returns:
        Optional[str]: 微博的mid
    """
    mid = select.xpath("//div[@mid]/@mid").get()
    return mid


@custom_validate_call
def get_uid(select: parsel.Selector) -> Optional[str]:
    """获取微博的uid

    Args:
        select (parsel.Selector): 经过 parsel 解析 html 后得到的 Selector 对象

    Returns:
        Optional[str]: 微博的uid
    """
    uid = select.xpath("//a[@nick-name]/@href").get()
    if uid is None:
        return None
    else:
        uid = re.search(r"/(\d+)/?", uid).group(1)
        return uid


@custom_validate_call
def get_personal_name(select: parsel.Selector) -> Optional[str]:
    """获取微博的个人名称

    Args:
        select (parsel.Selector): 经过 parsel 解析 html 后得到的 Selector 对象

    Returns:
        Optional[str]: 微博的个人名称
    """
    personal_name = select.xpath("//a[@nick-name]/@nick-name").get()
    return personal_name

@custom_validate_call
def get_personal_href(select: parsel.Selector) -> Optional[str]:
    """获取微博的个人主页

    Args:
        select (parsel.Selector): 经过 parsel 解析 html 后得到的 Selector 对象

    Returns:
        Optional[str]: 个人主页的 URL
    """
    personal_href = select.xpath("//a[@nick-name]/@href").get()
    if personal_href is None:
        return None
    else:
        return "https:" + personal_href


def get_weibo_href(select: parsel.Selector) -> Optional[str]:
    """获取微博的链接

    Args:
        select (parsel.Selector): 经过 parsel 解析 html 后得到的 Selector 对象

    Returns:
        Optional[str]: 微博的链接
    """
    weibo_href = select.xpath('//div[@class="from"]/a[1]/@href').get()
    if weibo_href is None:
        return None
    else:
        return "https:" + weibo_href


@custom_validate_call
def get_publish_time(select: parsel.Selector) -> Optional[str]:
    """获取微博的发布时间

    Args:
        select (parsel.Selector): 经过 parsel 解析 html 后得到的 Selector 对象

    Returns:
        Optional[datetime]: 微博的发布时间
    """
    publish_time_str = select.xpath('//div[@class="from"]/a[1]/text()').get()
    if publish_time_str is None:
        return publish_time_str
    else:
        publish_time = process_time_str(publish_time_str).strftime("%Y-%m-%d %H:%M:%S")
        return publish_time

@custom_validate_call
def get_content_from(select:parsel.Selector) -> Optional[str]:
    """获取微博的发送设备

    Args:
        select (parsel.Selector): 经过 parsel 解析 html 后得到的 Selector 对象

    Returns:
        Optional[str]: 微博的发送设备
    """
    content_from = select.xpath('//div[@class="from"]/a[2]/text()').get()
    return content_from

@custom_validate_call
def get_content_all(select:parsel.Selector) -> Optional[str]:
    """获取微博的内容

    Args:
        select (parsel.Selector): 经过 parsel 解析 html 后得到的 Selector 对象

    Returns:
        Optional[str]: 微博的内容
    """
    content_all = select.xpath('string(//p[@node-type="feed_list_content_full"])').get()
    content_all = re.sub(r"\n[ \t]+", "\n", content_all)
    content_all = re.sub(r"(?<!\n)\n(?!\n)", "", content_all)
    content_all = re.sub(r"[ \t]*收起d[ \t]*", "", content_all)

    content_show = select.xpath('string(//p[@node-type="feed_list_content"])').get()
    content_show = re.sub(r"\n[ \t]+", "\n", content_show)
    content_show = re.sub(r"(?<!\n)\n(?!\n)", "", content_show)
    
    content_final = content_all if content_all else content_show
    content_final = content_final.replace("\u200b", "").strip()
    content_final = re.sub(r"[ \t]*\n+[ \t]*", "\n\n", content_final)

    return content_final

@custom_validate_call
def get_retweet_num(select: parsel.Selector) -> Optional[int]:
    """获取微博的转发数量

    Args:
        select (parsel.Selector): 经过 parsel 解析 html 后得到的 Selector 对象

    Returns:
        Optional[int]: 微博的转发数量
    """
    retweet_num = select.xpath('string(//div[@class="card-act"]/ul[1]/li[1])').get()
    if retweet_num:
        retweet_num = re.findall(r"\d+", retweet_num)
        return int(retweet_num[0]) if retweet_num else 0
    else:
        return None
        
    
@custom_validate_call
def get_comment_num(select:parsel.Selector) -> Optional[int]:
    """获取微博的评论数量

    Args:
        select (parsel.Selector): 经过 parsel 解析 html 后得到的 Selector 对象

    Returns:
        Optional[int]: 微博的评论数量
    """
    comment_num = select.xpath('string(//div[@class="card-act"]/ul[1]/li[2])').get()
    if comment_num:
        comment_num = re.findall(r"\d+", comment_num)
        return int(comment_num[0]) if comment_num else 0
    else:
        return None

@custom_validate_call
def get_star_num(select: parsel.Selector) -> Optional[int]:
    """获取微博的点赞数量

    Args:
        select (parsel.Selector): 经过 parsel 解析 html 后得到的 Selector 对象

    Returns:
        Optional[int]: 微博的点赞数量
    """
    star_num = select.xpath('string(//div[@class="card-act"]/ul[1]/li[3])').get()
    if star_num:
        star_num = re.findall(r"\d+", star_num)
        return int(star_num[0]) if star_num else 0
    else:
        return None
    

# @custom_validate_call
def parse_list_html(html: str) -> List[dict]:
    """解析微博列表主体的html

    Args:
        html (str): 爬虫获取到的 html 文本

    Returns:
        List[dict]: 整理后的 List[dict]
    """
    select = parsel.Selector(html)
    div_list = select.xpath('//*[@id="pl_feedlist_index"]//div[@action-type="feed_list_item"]').getall()
    lst = []
    for div_string in div_list:
        select = parsel.Selector(div_string)
        item = {
            "mid": get_mid(select),
            "uid": get_uid(select),
            "个人昵称": get_personal_name(select),
            "个人主页": get_personal_href(select),
            "微博主页": get_weibo_href(select),
            "发布时间": get_publish_time(select),
            "内容来自": get_content_from(select),
            "全部内容": get_content_all(select),
            "转发数量": get_retweet_num(select),
            "评论数量": get_comment_num(select),
            "点赞数量": get_star_num(select),
        }
        lst.append(item)

    return lst