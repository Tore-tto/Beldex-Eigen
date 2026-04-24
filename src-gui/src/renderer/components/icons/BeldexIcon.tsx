

export default function BeldexIcon(props: any) {
  return (
    <img
      src="/assets/bdx_icon.png"
      alt="Beldex Icon"
      style={{
        width: "1.1em",
        height: "1.1em",
        verticalAlign: "middle",
        ...props.style,
      }}
      {...props}
    />
  );
}
