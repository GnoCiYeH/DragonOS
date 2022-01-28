//
// Created by longjin on 2022/1/22.
//
#include "printk.h"
//#include "linkage.h"

struct screen_info pos;

void show_color_band(int width, int height, char a, char b, char c, char d)
{
    /** 向帧缓冲区写入像素值
     * @param address: 帧缓存区的地址
     * @param val:像素值
     */

    for (int i = 0; i < width * height; ++i)
    {

        *((char *)pos.FB_address + 0) = d;
        *((char *)pos.FB_address + 1) = c;
        *((char *)pos.FB_address + 2) = b;
        *((char *)pos.FB_address + 3) = a;
        ++pos.FB_address;
    }
}

int calculate_max_charNum(int len, int size)
{
    /**
     * @brief 计算屏幕上能有多少行
     * @param len 屏幕长/宽
     * @param size 字符长/宽
     */
    return len / size;
}

int init_printk(const int width, const int height, unsigned int *FB_address, const int FB_length, const int char_size_x, const int char_size_y)
{

    pos.width = width;
    pos.height = height;
    pos.char_size_x = char_size_x;
    pos.char_size_y = char_size_y;
    pos.max_x = calculate_max_charNum(width, char_size_x);
    pos.max_y = calculate_max_charNum(height, char_size_y);

    pos.FB_address = FB_address;
    pos.FB_length = FB_length;

    pos.x = 0;
    pos.y = 0;

    return 0;
}

int set_printk_pos(const int x, const int y)
{
    // 指定的坐标不在屏幕范围内
    if (!((x >= 0 && x <= pos.max_x) && (y >= 0 && y <= pos.max_y)))
        return EPOS_OVERFLOW;
    pos.x = x;
    pos.y = y;
    return 0;
}
int skip_and_atoi(const char **s)
{
    /**
     * @brief 获取连续的一段字符对应整数的值
     * @param:**s 指向 指向字符串的指针 的指针
     */
    int ans = 0;
    while (is_digit(**s))
    {
        ans = ans * 10 + (**s) - '0';
        ++(*s);
    }
    return ans;
}

void auto_newline()
{
    /**
     * @brief 超过每行最大字符数，自动换行
     * 
     */
    if (pos.x > pos.max_x)
    {
        pos.x = 0;
        ++pos.y;
    }
    if (pos.y > pos.max_y)
        pos.y = 0;
}

static int vsprintf(char *buf, const char *fmt, va_list args)
{
    /**
     * 将字符串按照fmt和args中的内容进行格式化，然后保存到buf中
     * @param buf 结果缓冲区
     * @param fmt 格式化字符串
     * @param args 内容
     * @return 最终字符串的长度
     */

    char *str, *s;

    str = buf;

    int flags;       // 用来存储格式信息的bitmap
    int field_width; //区域宽度
    int precision;   //精度
    int qualifier;   //数据显示的类型
    int len;

    //开始解析字符串
    for (; *fmt; ++fmt)
    {
        //内容不涉及到格式化，直接输出
        if (*fmt != '%')
        {
            *str = *fmt;
            ++str;
            continue;
        }

        //开始格式化字符串

        //清空标志位和field宽度
        field_width = flags = 0;

        bool flag_tmp = true;
        bool flag_break = false;

        ++fmt;
        while (flag_tmp)
        {
            switch (*fmt)
            {
            case '\0':
                //结束解析
                flag_break = true;
                flag_tmp = false;
                break;

            case '-':
                // 左对齐
                flags |= LEFT;
                ++fmt;
                break;
            case '+':
                //在正数前面显示加号
                flags |= PLUS;
                ++fmt;
                break;
            case ' ':
                flags |= SPACE;
                ++fmt;
                break;
            case '#':
                //在八进制数前面显示 '0o'，在十六进制数前面显示 '0x' 或 '0X'
                flags |= SPECIAL;
                ++fmt;
                break;
            case '0':
                //显示的数字之前填充‘0’来取代空格
                flags |= PAD_ZERO;
                ++fmt;
                break;
            default:
                flag_tmp = false;
                break;
            }
        }
        if (flag_break)
            break;

        //获取区域宽度
        field_width = -1;
        if (*fmt == '*')
        {
            field_width = va_arg(args, int);
            ++fmt;
        }
        else if (is_digit(*fmt))
        {
            field_width = skip_and_atoi(&fmt);
            if (field_width < 0)
            {
                field_width = -field_width;
                flags |= LEFT;
            }
        }

        //获取小数精度
        precision = -1;
        if (*fmt == '.')
        {
            ++fmt;
            if (*fmt == '*')
            {
                precision = va_arg(args, int);
                ++fmt;
            }
            else if is_digit (*fmt)
            {
                precision = skip_and_atoi(&fmt);
            }
        }

        //获取要显示的数据的类型
        if (*fmt == 'h' || *fmt == 'l' || *fmt == 'L' || *fmt == 'Z')
        {
            qualifier = *fmt;
            ++fmt;
        }
        //为了支持lld
        if (qualifier == 'l' && *fmt == 'l', *(fmt + 1) == 'd')
            ++fmt;

        //转化成字符串
        long long *ip;
        switch (*fmt)
        {
        //输出 %
        case '%':
            *str++ = '%';

            break;
        // 显示一个字符
        case 'c':
            //靠右对齐
            if (!(flags & LEFT))
            {
                while (--field_width > 0)
                {
                    *str = ' ';
                    ++str;
                }
            }

            *str++ = (unsigned char)va_arg(args, int);

            while (--field_width > 0)
            {
                *str = ' ';
                ++str;
            }

            break;

        //显示一个字符串
        case 's':
            s = va_arg(args, char *);
            if (!s)
                s = '\0';
            len = strlen(s);
            if (precision < 0)
            {
                //未指定精度
                precision = len;
            }

            else if (len > precision)
            {
                len = precision;
            }

            //靠右对齐
            if (!(flags & LEFT))
                while (len < field_width--)
                {
                    *str = ' ';
                    ++str;
                }

            for (int i = 0; i < len; i++)
            {
                *str = *s;
                ++s;
                ++str;
            }

            while (len < field_width--)
            {
                *str = ' ';
                ++str;
            }

            break;
        //以八进制显示字符串
        case 'o':
            flags |= SMALL;
        case 'O':
            flags |= SPECIAL;
            if (qualifier == 'l')
                str = write_num(str, va_arg(args, long long), 8, field_width, precision, flags);
            else
                str = write_num(str, va_arg(args, int), 8, field_width, precision, flags);
            break;

        //打印指针指向的地址
        case 'p':
            if (field_width == 0)
            {
                field_width = 2 * sizeof(void *);
                flags |= PAD_ZERO;
            }

            str = write_num(str, (unsigned long)va_arg(args, void *), 16, field_width, precision, flags);

            break;

        //打印十六进制
        case 'x':
            flags |= SMALL;
        case 'X':
            //flags |= SPECIAL;
            if (qualifier == 'l')
                str = write_num(str, va_arg(args, long long), 16, field_width, precision, flags);
            else
                str = write_num(str, va_arg(args, int), 16, field_width, precision, flags);
            break;

        //打印十进制有符号整数
        case 'i':
        case 'd':

            flags |= SIGN;
            if (qualifier == 'l')
                str = write_num(str, va_arg(args, long long), 10, field_width, precision, flags);
            else
                str = write_num(str, va_arg(args, int), 10, field_width, precision, flags);
            break;

        //打印十进制无符号整数
        case 'u':
            if (qualifier == 'l')
                str = write_num(str, va_arg(args, unsigned long long), 10, field_width, precision, flags);
            else
                str = write_num(str, va_arg(args, unsigned int), 10, field_width, precision, flags);
            break;

        //输出有效字符数量到*ip对应的变量
        case 'n':

            if (qualifier == 'l')
                ip = va_arg(args, long long *);
            else
                ip = (ll*)va_arg(args, int *);

            *ip = str - buf;
            break;
        case 'f':
            // 默认精度为3
                //printk("1111\n");
                //va_arg(args, double);
                //printk("222\n");
                
            if (precision < 0)
                precision = 3;
                str = write_float_point_num(str, va_arg(args, double), field_width, precision, flags);
               
            
            break;

        //对于不识别的控制符，直接输出
        default:
            *str++ = '%';
            if (*fmt)
                *str++ = *fmt;
            else
                --fmt;
            break;
        }
    }
    *str = '\0';

    //返回缓冲区已有字符串的长度。
    return str - buf;
}

static char *write_num(char *str, long long num, int base, int field_width, int precision, int flags)
{
    /**
     * @brief 将数字按照指定的要求转换成对应的字符串
     * 
     * @param str 要返回的字符串
     * @param num 要打印的数值
     * @param base 基数
     * @param field_width 区域宽度 
     * @param precision 精度
     * @param flags 标志位
     */

    // 首先判断是否支持该进制
    if (base < 2 || base > 36)
        return 0;
    char pad, sign, tmp_num[100];

    const char *digits = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    // 显示小写字母
    if (flags & SMALL)
        digits = "0123456789abcdefghijklmnopqrstuvwxyz";

    if(flags&LEFT)
        flags &= ~PAD_ZERO;
    // 设置填充元素
    pad = (flags & PAD_ZERO) ? '0' : ' ';


    sign = 0;
    if (flags & SIGN && num < 0)
    {
        sign = '-';
        num = -num;
    }
    else
    {
        // 设置符号
        sign = (flags & PLUS) ? '+' : ((flags & SPACE) ? ' ' : 0);
    }

    // sign占用了一个宽度
    if (sign)
        --field_width;

    if (flags & SPECIAL)
        if (base == 16) // 0x占用2个位置
            field_width -= 2;
        else if (base == 8) // O占用一个位置
            --field_width;

    int js_num = 0; // 临时数字字符串tmp_num的长度

    if (num == 0)
        tmp_num[js_num++] = '0';
    else
    {
        num = ABS(num);
        //进制转换
        while (num > 0)
        {
            tmp_num[js_num++] = digits[num % base]; // 注意这里，输出的数字，是小端对齐的。低位存低位
            num /= base;
        }
    }

    if (js_num > precision)
        precision = js_num;

    field_width -= precision;

    // 靠右对齐
    if (!(flags & (LEFT+PAD_ZERO)))
        while (field_width-- > 0)
            *str++ = ' ';

    if (sign)
        *str++ = sign;
    if (flags & SPECIAL)
        if (base == 16)
        {
            *str++ = '0';
            *str++ = digits[33];
        }
        else if (base == 8)
            *str++ = digits[24]; //注意这里是英文字母O或者o
    if(!(flags&LEFT))
        while(field_width-->0)
            *str++ = pad;
    while (js_num < precision)
    {
        --precision;
        *str++ = '0';
    }

    while (js_num-- > 0)
        *str++ = tmp_num[js_num];

    while (field_width-- > 0)
        *str++ = ' ';

    return str;
}

static char *write_float_point_num(char *str, double num, int field_width, int precision, int flags)
{
    /**
     * @brief 将浮点数按照指定的要求转换成对应的字符串
     * 
     * @param str 要返回的字符串
     * @param num 要打印的数值
     * @param field_width 区域宽度 
     * @param precision 精度
     * @param flags 标志位
     */

    char pad, sign, tmp_num_z[100], tmp_num_d[350];

    const char *digits = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    // 显示小写字母
    if (flags & SMALL)
        digits = "0123456789abcdefghijklmnopqrstuvwxyz";

    // 设置填充元素
    pad = (flags & PAD_ZERO) ? '0' : ' ';

    sign = 0;
    if (flags & SIGN && num < 0)
    {
        sign = '-';
        num = -num;
    }
    else
    {
        // 设置符号
        sign = (flags & PLUS) ? '+' : ((flags & SPACE) ? ' ' : 0);
    }

    // sign占用了一个宽度
    if (sign)
        --field_width;

    int js_num_z = 0, js_num_d = 0;                          // 临时数字字符串tmp_num_z tmp_num_d的长度
    ul num_z = (ul)(num);                                    // 获取整数部分
    ul num_decimal = (ul)(round((num - num_z) * precision)); // 获取小数部分

    if (num == 0)
        tmp_num_z[js_num_z++] = '0';
    else
    {
        //存储整数部分
        while (num_z > 0)
        {
            tmp_num_z[js_num_z++] = digits[num_z % 10]; // 注意这里，输出的数字，是小端对齐的。低位存低位
            num_z /= 10;
        }
    }

    while (num_decimal > 0)
    {
        tmp_num_d[js_num_d++] = digits[num_decimal % 10];
        num_decimal /= 10;
    }

    field_width -= (precision + 1 + js_num_z);

    // 靠右对齐
    if (!(flags & LEFT))
        while (field_width-- > 0)
            *str++ = pad;

    if (sign)
        *str++ = sign;

    // 输出整数部分
    while (js_num_z-- > 0)
        *str++ = tmp_num_z[js_num_z];

    *str++ = '.';

    // 输出小数部分
    while (js_num_d-- > 0)
        *str++ = tmp_num_d[js_num_d];

    while (js_num_d < precision)
    {
        --precision;
        *str++ = '0';
    }

    while (field_width-- > 0)
        *str++ = ' ';

    return str;
}

static void putchar(unsigned int *fb, int Xsize, int x, int y, unsigned int FRcolor, unsigned int BKcolor, unsigned char font)
{
    /**
     * @brief 在屏幕上指定位置打印字符
     * 
     * @param fb 帧缓存线性地址
     * @param Xsize 行分辨率
     * @param x 左上角列像素点位置
     * @param y 左上角行像素点位置
     * @param FRcolor 字体颜色
     * @param BKcolor 背景颜色
     * @param font 字符的bitmap
     */

    unsigned char *font_ptr = font_ascii[font];
    unsigned int *addr;

    int testbit; // 用来测试某位是背景还是字体本身

    for (int i = 0; i < pos.char_size_y; ++i)
    {
        // 计算出帧缓冲区的地址
        addr = fb + Xsize * (y + i) + x;
        testbit = (1 << (pos.char_size_x + 1));
        for (int j = 0; j < pos.char_size_x; ++j)
        {
            //从左往右逐个测试相应位
            testbit >>= 1;
            if (*font_ptr & testbit)
                *addr = FRcolor; // 字，显示前景色
            else
                *addr = BKcolor; // 背景色

            ++addr;
        }
        ++font_ptr;
    }
}

int printk_color(unsigned int FRcolor, unsigned int BKcolor, const char *fmt, ...)
{
    /**
     * @brief 格式化打印字符串
     * 
     * @param FRcolor 前景色
     * @param BKcolor 背景色
     * @param ... 格式化字符串
     */

    va_list args;
    va_start(args, fmt);

    int len = vsprintf(buf, fmt, args);

    va_end(args);
    unsigned char current;

    int i; // 总共输出的字符数
    for (i = 0; i < len; ++i)
    {
        current = *(buf + i);
        //输出换行
        if (current == '\n')
        {
            pos.x = 0;
            ++pos.y;
        }
        else if (current == '\t') // 输出制表符
        {
            int space_to_print = 8 - pos.x % 8;

            while (space_to_print--)
            {
                putchar(pos.FB_address, pos.width, pos.x * pos.char_size_x, pos.y * pos.char_size_y, BLACK, BLACK, ' ');
                ++pos.x;

                auto_newline();
            }
        }
        else if (current == '\b') // 退格
        {
            --pos.x;
            if (pos.x < 0)
            {
                --pos.y;
                if (pos.y <= 0)
                    pos.x = pos.y = 0;
                else
                    pos.x = pos.max_x;
            }

            putchar(pos.FB_address, pos.width, pos.x * pos.char_size_x, pos.y * pos.char_size_y, FRcolor, BKcolor, ' ');
            ++pos.x;

            auto_newline();
        }
        else
        {
            putchar(pos.FB_address, pos.width, pos.x * pos.char_size_x, pos.y * pos.char_size_y, FRcolor, BKcolor, current);
            ++pos.x;
            auto_newline();
        }
    }

    return i;
}