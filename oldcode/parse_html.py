# parse_html.py

from parsel import Selector
import pandas as pd
import re
import time


def get_html_text(file_path):
    with open(file_path, mode="r", encoding="utf-8") as f:
        html_text = f.read()
    return html_text


def parse_html(html_text):
    lst = []
    select = Selector(html_text)
    select.xpath("//i").drop()

    # 获取总页数
    total_page = select.xpath(
        '//ul[@node-type="feed_list_page_morelist"]/li/a/text()'
    ).getall()[-1]
    total_page = int(re.findall(r"[\d]+", total_page)[0])

    # 获取主体信息
    div_list = select.xpath(
        '//*[@id="pl_feedlist_index"]//div[@action-type="feed_list_item"]'
    ).getall()

    for div_string in div_list:
        select = Selector(div_string)
        mid = select.xpath("//div[@mid]/@mid").get()
        personal_name = select.xpath("//a[@nick-name]/@nick-name").get()
        personal_href = select.xpath("//a[@nick-name]/@href").get()
        publish_time = select.xpath('//div[@class="from"]/a[1]/text()').get()
        content_from = select.xpath('//div[@class="from"]/a[2]/text()').get()
        content_show = select.xpath('string(//p[@node-type="feed_list_content"])').get()
        content_all = select.xpath(
            'string(//p[@node-type="feed_list_content_full"])'
        ).get()
        retweet_num = select.xpath('string(//div[@class="card-act"]/ul[1]/li[1])').get()
        comment_num = select.xpath('string(//div[@class="card-act"]/ul[1]/li[2])').get()
        star_num = select.xpath('string(//div[@class="card-act"]/ul[1]/li[3])').get()
        item = [
            mid,
            personal_name,
            personal_href,
            publish_time,
            content_from,
            content_show,
            content_all,
            retweet_num,
            comment_num,
            star_num,
        ]
        lst.append(item)

    columns = [
        "mid",
        "个人昵称",
        "个人主页",
        "发布时间",
        "内容来自",
        "展示内容",
        "全部内容",
        "转发数量",
        "评论数量",
        "点赞数量",
    ]
    data = pd.DataFrame(lst, columns=columns)

    return data, total_page


def process_time(publish_time):
    now = int(time.time())
    publish_time = publish_time.strip()
    if "人数" in publish_time:
        publish_time = " ".join(publish_time.split()[:-1])
        publish_time = publish_time.replace("今天 ", "今天")
    if "分钟前" in publish_time:
        minutes = re.findall(r"([\d].*)分钟前", publish_time)[0]
        now = now - int(minutes) * 60
        timeArray = time.localtime(now)
        publish_time = time.strftime("%m月%d日 %H:%M", timeArray)
    if "今天" in publish_time:
        timeArray = time.localtime(now)
        today = time.strftime("%m月%d日 ", timeArray)
        publish_time = publish_time.replace("今天", today)
    return publish_time


def process_dataframe(data):
    data.insert(
        1, "uid", data["个人主页"].map(lambda href: re.findall(r"com/(.*)\?", href)[0])
    )
    data["个人主页"] = "https:" + data["个人主页"]
    data["个人主页"] = data["个人主页"].map(lambda x: re.findall(r"(.*)\?", x)[0])
    data["发布时间"] = data["发布时间"].map(process_time)
    data.iloc[:, 6:] = data.iloc[:, 6:].applymap(
        lambda x: x.replace("\n", "").replace(" ", "") if x else None
    )  # 清楚掉 \n 和 空格
    data["全部内容"] = data["全部内容"].map(
        lambda x: x[:-2] if x else None
    )  # 清除掉收起
    data.iloc[:, -3:] = data.iloc[:, -3:].applymap(
        lambda x: 0 if x in ["转发", "评论", "赞"] else x
    )
    return data


def get_dataframe_from_html_text(html_text):
    data, total_page = parse_html(html_text)
    process_dataframe(data)
    return data, total_page