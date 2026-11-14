/**
 * 搜索选择:输入关键词搜索歌单或歌曲,点击结果选择
 *   new SearchPicker({
 *       el: "#picker",
 *       url: "/admin/api/common/favorite_search",   //返回[{id, name, logoUrl, memo}]
 *       placeholder: "输入歌单名或ID搜索",
 *       onPick: item => {}
 *   })
 */
class SearchPicker {
    constructor(options) {
        this.options = options
        this.list = []
        this.timer = null
        this.$el = $(options.el)
        this.$el.html(`<input class="form-control" autocomplete="off" placeholder="${escapeHtml(options.placeholder || "")}" style="max-width: 420px">
                       <div class="picker-result list-group mt-1 d-none"></div>`)
        this.$input = this.$el.find("input")
        this.$result = this.$el.find(".picker-result")

        //停止输入0.3秒后搜索;点击输入框时显示最新的
        this.$input.on("input", () => {
            clearTimeout(this.timer)
            this.timer = setTimeout(() => this.search(), 300)
        })
        this.$input.on("focus", () => this.search())

        //在搜索框里按回车不提交表单
        this.$input.on("keydown", e => {
            if (e.key === "Enter") {
                e.preventDefault()
                clearTimeout(this.timer)
                this.search()
            }
        })
        this.$result.on("click", "[data-index]", e => {
            const item = this.list[parseInt($(e.currentTarget).data("index"))]
            this.close()
            this.$input.val("")
            this.options.onPick(item)
        })

        //点击其他地方时收起结果
        $(document).on("click", e => {
            if (!$(e.target).closest(this.$el).length) {
                this.close()
            }
        })
    }

    search() {
        const key = this.$input.val().trim()
        $.ajaxByData(this.options.url).add("key", key).hide().success(list => {
            //只显示最后一次输入的结果
            if (key !== this.$input.val().trim()) {
                return
            }
            this.list = list
            let html = ""
            list.forEach((it, i) => {
                html += `<button type="button" class="list-group-item list-group-item-action d-flex align-items-center gap-2 py-2" data-index="${i}">${pickItemHtml(it)}</button>`
            })
            if (list.length === 0) {
                html = `<div class="list-group-item text-secondary small">没有找到</div>`
            }
            this.$result.html(html).removeClass("d-none")
        }).get()
    }

    close() {
        this.$result.addClass("d-none")
    }
}

/**
 * 一个歌单或歌曲的显示内容:封面、名称、ID和补充说明
 */
function pickItemHtml(it) {
    return `${logoHtml(it.logoUrl, 36)}
            <div style="min-width: 0">
                <div class="text-truncate">${escapeHtml(it.name)}</div>
                <div class="small text-secondary text-truncate">ID:${it.id} ${escapeHtml(it.memo || "")}</div>
            </div>`
}
