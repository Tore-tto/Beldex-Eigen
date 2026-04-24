

export default function BeldexIcon(props: any) {
  return (
    <img
      src="/assets/bdx_icon.png"
      alt="Beldex Icon"
      style={{
        width: "38px",
        height: "38px",
        verticalAlign: "middle",
        ...props.style,
      }}
      {...props}
    />
  );
}
