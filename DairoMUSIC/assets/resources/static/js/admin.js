/**
 * 后台管理公共脚本
 */

/** 菜单缓存的key,页面切换时先用缓存显示,避免菜单闪烁 */
const MENU_CACHE_KEY = "adminMenu"

$(function () {
    if ($(".app-sidebar").length > 0) {
        initLayout()
    }
})

/**
 * 初始化左侧菜单栏和顶部栏
 */
function initLayout() {
    $("#appTitle").text(document.title)

    //先用缓存的菜单显示,之后再从服务器获取最新的
    try {
        const cache = sessionStorage.getItem(MENU_CACHE_KEY)
        if (cache) {
            renderMenu(JSON.parse(cache))
        }
    } catch {
    }
    $.ajaxByData("/admin/api/common/menu").hide().success(data => {
        try {
            sessionStorage.setItem(MENU_CACHE_KEY, JSON.stringify(data))
        } catch {
        }
        renderMenu(data)
    }).get()
    $.ajaxByData("/admin/api/common/info").hide().success(data => {
        const name = data.realName || data.name
        $("#appUserName").text(name)
        $("#appAvatar").text(name.substring(0, 1).toUpperCase())
    }).get()
}

/**
 * 当前页面对应的菜单链接
 * 编辑页等不在菜单里的页面,通过 <meta name="menu" content="/admin/xxx_list.html"> 指定高亮哪个菜单
 */
function currentMenuUrl() {
    const meta = $("meta[name='menu']").attr("content")
    return meta || location.pathname
}

/**
 * 显示菜单
 * @param menus 两级菜单
 */
function renderMenu(menus) {
    const current = currentMenuUrl()
    let html = ""
    menus.forEach(menu => {
        const icon = `<i class="bi bi-${escapeHtml(menu.ico || "circle")}"></i>`
        const name = `<span>${escapeHtml(menu.name)}</span>`
        if (menu.children.length === 0) {
            const active = menu.url === current ? " active" : ""
            html += `<a class="app-menu-item${active}" href="${escapeHtml(menu.url)}">${icon}${name}</a>`
            return
        }
        const isOpen = menu.children.some(it => it.url === current)
        const groupId = "appMenuGroup" + menu.id
        html += `<button class="app-menu-item" type="button" data-bs-toggle="collapse" data-bs-target="#${groupId}" aria-expanded="${isOpen}">
                    ${icon}${name}<i class="bi bi-chevron-right app-menu-arrow"></i>
                 </button>
                 <div id="${groupId}" class="collapse${isOpen ? " show" : ""}">
                    <div class="app-submenu">`
        menu.children.forEach(it => {
            const active = it.url === current ? " active" : ""
            html += `<a class="app-menu-item${active}" href="${escapeHtml(it.url || "#")}">${escapeHtml(it.name)}</a>`
        })
        html += `</div></div>`
    })
    $("#appMenu").html(html)
}

/**
 * 手机端显示/隐藏左侧菜单栏
 * @param show 是否显示,不传时切换
 */
function toggleSidebar(show) {
    $("body").toggleClass("sidebar-open", show)
}

/**
 * 退出登录
 */
function logout() {
    $.ajaxByData("/admin/api/login/logout").success(() => {
        try {
            sessionStorage.removeItem(MENU_CACHE_KEY)
        } catch {
        }
        location.href = "/admin/login.html"
    }).delete()
}

/**
 * 获取url参数
 * @param key 参数名
 * @param def 参数不存在时的默认值
 * @returns {string}
 */
function getParam(key, def = "") {
    const value = new URLSearchParams(location.search).get(key)
    return value == null ? def : value
}

/**
 * HTML转义,把用户输入的内容拼接到HTML里时使用
 * @param str 要转义的字符串
 * @returns {string}
 */
function escapeHtml(str) {
    if (str == null) {
        return ""
    }
    return String(str)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;")
}

/**
 * 毫秒时间戳格式化成 yyyy-MM-dd HH:mm:ss
 * @param millis 毫秒时间戳,为空或0时返回空字符串
 * @returns {string}
 */
function formatTime(millis) {
    if (!millis) {
        return ""
    }
    const d = new Date(millis)
    const pad = n => String(n).padStart(2, "0")
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

/**
 * 数据大小格式化
 * @param size 字节数
 * @returns {string}
 */
function formatSize(size) {
    if (size == null) {
        return ""
    }
    const units = ["B", "KB", "MB", "GB", "TB"]
    let value = size
    let i = 0
    while (value >= 1024 && i < units.length - 1) {
        value /= 1024
        i++
    }
    return (i === 0 ? value : value.toFixed(2)) + units[i]
}

/**
 * 数据表格(服务端分页)
 * 用法:
 *   const table = new PageTable({
 *       el: "#table",                 //表格容器
 *       url: "/admin/api/xxx/list",   //数据接口,返回 {total, list} ;paging为false时返回数组
 *       search: "#search",            //搜索表单(可选),表单提交时重新查询
 *       paging: true,                 //是否分页
 *       pageSize: 20,
 *       columns: [{title: "标题", data: "字段名", render: item => "html", sortKey: "排序字段", className: ""}],
 *       actions: [{icon: "bi-pencil-square", title: "编辑", onClick: item => {}, show: item => true}],
 *       selectable: false,            //是否显示勾选框,通过table.selected()获取勾选的数据
 *       autoSearch: true,             //搜索条件变化时自动查询(输入框停止输入0.4秒后,下拉框、开关改变后),不需要查询按钮
 *       empty: "暂无数据"
 *   })
 *   table.load()
 */
class PageTable {
    constructor(options) {
        this.options = Object.assign({paging: true, pageSize: 20, actions: [], empty: "暂无数据", autoSearch: true}, options)
        this.$el = $(options.el)
        this.page = 1
        this.sortKey = ""
        this.sortType = ""
        this.list = []
        if (options.search) {
            $(options.search).on("submit", e => {
                e.preventDefault()
                this.load(1)
            })
            if (this.options.autoSearch) {
                let timer = null
                const search = () => {
                    clearTimeout(timer)

                    //条件和上次查询的一样时不查询(比如输入后又删掉)
                    if (JSON.stringify(this.searchParam()) !== this.lastSearch) {
                        this.load(1)
                    }
                }
                $(options.search).on("input", "input:not([type=checkbox]):not([type=radio])", () => {
                    clearTimeout(timer)
                    timer = setTimeout(search, 400)
                })
                $(options.search).on("change", "select, input[type=checkbox], input[type=radio]", search)

                //按回车已经查询过了,不用再自动查询一次
                $(options.search).on("submit", () => clearTimeout(timer))
            }
        }
        this.$el.on("click", "th[data-sort]", e => {
            const key = $(e.currentTarget).data("sort")
            if (this.sortKey === key) {
                this.sortType = this.sortType === "asc" ? "desc" : "asc"
            } else {
                this.sortKey = key
                this.sortType = "desc"
            }
            this.load(1)
        })
        this.$el.on("click", "[data-action]", e => {
            const $btn = $(e.currentTarget)
            const item = this.list[parseInt($btn.data("row"))]
            this.options.actions[parseInt($btn.data("action"))].onClick(item)
        })
        this.$el.on("change", "input[data-select-all]", e => {
            this.$el.find("input[data-select-row]").prop("checked", e.currentTarget.checked)
        })
        this.$el.on("click", ".page-link[data-page]", e => {
            e.preventDefault()
            const page = parseInt($(e.currentTarget).data("page"))
            if (page > 0) {
                this.load(page)
            }
        })
    }

    /**
     * 获取搜索条件(空值不提交)
     */
    searchParam() {
        const param = {}
        if (!this.options.search) {
            return param
        }
        const data = $(this.options.search).formData()
        Object.keys(data).forEach(key => {
            if (data[key] !== "" && data[key] != null) {
                param[key] = data[key]
            }
        })
        return param
    }

    /**
     * 加载数据
     * @param page 页码,不传时重新加载当前页
     */
    load(page) {
        if (page) {
            this.page = page
        }
        const param = this.searchParam()

        //记录查询条件,自动查询时用来判断条件有没有变化
        this.lastSearch = JSON.stringify(param)
        const http = $.ajaxByData(this.options.url).addAll(param)
        if (this.options.paging) {
            http.add("page", this.page).add("pageSize", this.options.pageSize)
        }
        if (this.sortKey) {
            http.add("sortKey", this.sortKey).add("sortType", this.sortType)
        }
        http.success(data => {
            if (this.options.paging) {
                this.render(data.list, data.total)
            } else {
                this.render(data, data.length)
            }
        }).get()
    }

    /**
     * 勾选的数据(selectable为true时)
     */
    selected() {
        return this.$el.find("input[data-select-row]:checked").map((_, it) => this.list[parseInt($(it).data("select-row"))]).get()
    }

    render(list, total) {
        this.list = list
        const opt = this.options
        let html = `<div class="table-responsive"><table class="table table-hover align-middle"><thead><tr>`
        if (opt.selectable) {
            html += `<th style="width: 1px"><input class="form-check-input" type="checkbox" data-select-all title="全选"></th>`
        }
        opt.columns.forEach(col => {
            if (col.sortKey) {
                let icon = "bi-arrow-down-up text-body-tertiary"
                if (this.sortKey === col.sortKey) {
                    icon = this.sortType === "asc" ? "bi-sort-up" : "bi-sort-down"
                }
                html += `<th data-sort="${col.sortKey}" role="button" class="user-select-none ${col.className || ""}">${col.title}<i class="bi ${icon} ms-1 small"></i></th>`
            } else {
                html += `<th class="${col.className || ""}">${col.title}</th>`
            }
        })
        if (opt.actions.length > 0) {
            html += `<th class="text-end">操作</th>`
        }
        html += `</tr></thead><tbody>`
        if (list.length === 0) {
            const span = opt.columns.length + (opt.actions.length > 0 ? 1 : 0) + (opt.selectable ? 1 : 0)
            html += `<tr><td colspan="${span}" class="text-center text-secondary py-5"><i class="bi bi-inbox fs-3 d-block mb-1"></i>${opt.empty}</td></tr>`
        }
        list.forEach((item, row) => {
            html += `<tr>`
            if (opt.selectable) {
                html += `<td><input class="form-check-input" type="checkbox" data-select-row="${row}"></td>`
            }
            opt.columns.forEach(col => {
                const value = col.render ? col.render(item) : escapeHtml(item[col.data])
                html += `<td class="${col.className || ""}">${value}</td>`
            })
            if (opt.actions.length > 0) {
                html += `<td class="text-end text-nowrap">`
                opt.actions.forEach((action, i) => {
                    if (action.show && !action.show(item)) {
                        return
                    }
                    const color = action.danger ? "text-danger" : ""
                    html += `<button type="button" class="btn btn-sm btn-link px-1 ${color}" data-action="${i}" data-row="${row}" title="${action.title}"><i class="bi ${action.icon}"></i></button>`
                })
                html += `</td>`
            }
            html += `</tr>`
        })
        html += `</tbody></table></div>`
        if (opt.paging) {
            html += this.pagination(total)
        }
        this.$el.html(html)
    }

    /**
     * 分页按钮
     */
    pagination(total) {
        const pageCount = Math.max(1, Math.ceil(total / this.options.pageSize))
        const page = this.page
        const item = (p, text, disabled, active) =>
            `<li class="page-item${disabled ? " disabled" : ""}${active ? " active" : ""}"><a class="page-link" href="#" data-page="${disabled ? 0 : p}">${text}</a></li>`
        let html = `<div class="d-flex flex-wrap align-items-center justify-content-between gap-2 mt-3">
                        <div class="text-secondary small">共 ${total} 条,第 ${page}/${pageCount} 页</div>
                        <ul class="pagination pagination-sm mb-0">`
        html += item(page - 1, `<i class="bi bi-chevron-left"></i>`, page <= 1, false)
        const start = Math.max(1, page - 2)
        const end = Math.min(pageCount, page + 2)
        if (start > 1) {
            html += item(1, "1", false, false)
            if (start > 2) {
                html += item(0, "…", true, false)
            }
        }
        for (let p = start; p <= end; p++) {
            html += item(p, p, false, p === page)
        }
        if (end < pageCount) {
            if (end < pageCount - 1) {
                html += item(0, "…", true, false)
            }
            html += item(pageCount, pageCount, false, false)
        }
        html += item(page + 1, `<i class="bi bi-chevron-right"></i>`, page >= pageCount, false)
        html += `</ul></div>`
        return html
    }
}

/**
 * 状态标签
 * @param text 显示的文字
 * @param color 颜色 success/secondary/danger/warning/info/primary
 */
function badge(text, color = "secondary") {
    return `<span class="badge rounded-pill text-bg-${color} bg-opacity-75 fw-normal">${escapeHtml(text)}</span>`
}

/**
 * 有效/无效 状态标签
 */
function stateBadge(state) {
    return state === 1 ? badge("有效", "success") : badge("无效", "secondary")
}

/**
 * 填充下拉框选项
 * @param $select 下拉框
 * @param options [{value, text}]
 * @param first 第一个选项的文字(值为空),不传时没有
 */
function fillOptions($select, options, first) {
    let html = first === undefined ? "" : `<option value="">${escapeHtml(first)}</option>`
    options.forEach(it => {
        html += `<option value="${escapeHtml(it.value)}">${escapeHtml(it.text)}</option>`
    })
    $($select).html(html)
}

/**
 * 确认后执行
 * @param msg 提示信息
 * @param block 确认后执行的函数
 */
function confirmDo(msg, block) {
    if (confirm(msg)) {
        block()
    }
}

/**
 * 毫秒时长格式化成 分:秒
 */
function formatDuration(millis) {
    if (!millis) {
        return ""
    }
    const seconds = Math.round(millis / 1000)
    return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`
}

/**
 * 封面缩略图
 */
function logoHtml(url, size = 40) {
    if (!url) {
        return `<span class="d-inline-flex align-items-center justify-content-center rounded bg-body-secondary text-secondary" style="width:${size}px;height:${size}px"><i class="bi bi-music-note"></i></span>`
    }
    return `<img src="${escapeHtml(url)}" class="rounded object-fit-cover" style="width:${size}px;height:${size}px" alt="" loading="lazy">`
}

/** 歌曲状态 */
const MUSIC_STATES = [
    {value: 100, text: "采集完成"},
    {value: 0, text: "采集中"},
    {value: 1, text: "采集失败"}
]

/**
 * 歌曲审核状态(被拒绝的歌曲APP搜索不显示)
 */
const AUDIT_STATES = [
    {value: 0, text: "待审核"},
    {value: 1, text: "通过"},
    {value: 2, text: "拒绝"}
]

/**
 * 歌曲审核状态标签
 */
function auditStateBadge(state) {
    const colors = {0: "warning", 1: "success", 2: "danger"}
    const item = AUDIT_STATES.find(it => it.value === state)
    return item ? badge(item.text, colors[state]) : ""
}

/**
 * 歌曲状态标签
 */
function musicStateBadge(state) {
    const colors = {100: "success", 0: "info", 1: "danger"}
    const item = MUSIC_STATES.find(it => it.value === state)
    return item ? badge(item.text, colors[state]) : ""
}

/**
 * 音质显示
 */
function bitRateText(bitRate) {
    if (!bitRate) {
        return ""
    }
    if (bitRate >= 999) {
        return badge("无损", "primary")
    }
    return bitRate + "k"
}

/**
 * 过长时省略显示的文字,鼠标悬停时显示全部
 */
function ellipsis(text, maxWidth = 320) {
    if (text == null) {
        return ""
    }
    return `<div class="text-truncate" style="max-width:${maxWidth}px" title="${escapeHtml(text)}">${escapeHtml(text)}</div>`
}
